"""Generate GitHub-readable companions from immutable compiler snapshots.

No program execution, Python CBOR decoder, or hand-maintained second source.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def fields(text):
    result = {}
    while text.strip():
        text = text.lstrip()
        key, text = text.split('=', 1)
        if text.startswith(('"', '{', '[')):
            value, end = json.JSONDecoder().raw_decode(text)
            text = text[end:]
        else:
            value, _, text = text.partition(' ')
        result[key] = value
    return result


def type_text(ty):
    if ty is None: return ''
    if ty == 'None': return 'none'
    if 'Scalar' in ty:
        scalar, unit = ty['Scalar']
        return scalar + (f' [{unit}]' if unit else '')
    a = ty['Array']
    return ('mut ' if a['mutable'] else '') + f"Array<{a['element']},{a['rank']}>" + (f" [{a['unit']}]" if a['unit'] else '')


def pseudocode(records, namespace=''):
    def expr(path):
        kind, f = records[path]
        child = lambda suffix: expr(path + '/' + suffix)
        if kind == 'number':
            number = f['value']
            if f['scalar'] == 'f64' and re.fullmatch(r'-?\d+', number): number += '.0'
            return number + (':' + f['scalar'] if f['scalar'] not in ('f64', 'i64') else '') + (f" [{f['unit']}]" if f['unit'] != '1' else '')
        if kind == 'variable': return f['name']
        if kind == 'string':
            if 'hash' in f: return f"<string omitted by compiler query: {f['bytes']} bytes>"
            return json.dumps(f['value'], ensure_ascii=False)
        if kind == 'complex': return f"complex({f['real']}, {f['imag']})"
        if kind in ('binary', 'compare'): return f"({child('left')} {f['operator']} {child('right')})"
        if kind == 'unary': return f"({f['operator']}{child('value')})"
        if kind == 'index': return f"{child('array')}[{child('index')}]"
        if kind in ('array', 'call'):
            args = ', '.join(child(f'items/{i}') for i in range(int(f.get('elements', f.get('arguments', 0)))))
            name = f.get('function', '').removeprefix('@math.f64.')
            if namespace: name = name.removeprefix(namespace)
            return '[' + args + ']' if kind == 'array' else name + '(' + args + ')'
        raise ValueError(f'Unsupported expression: {kind}')

    lines = []
    def block(prefix, indent=0):
        paths = sorted((p for p in records if re.fullmatch(re.escape(prefix) + r'/\d+', p)), key=lambda p: int(p.rsplit('/', 1)[1]))
        if not paths: lines.append('    ' * indent + 'pass')
        for path in paths:
            kind, f = records[path]
            child = lambda suffix: expr(path + '/' + suffix)
            if kind == 'let':
                annotation = type_text(f['annotation']) if f['annotation'] != 'null' else ''
                text = ('var ' if f['mutable'] == 'true' else 'let ') + f['name'] + (': ' + annotation if annotation else '') + ' = ' + child('value')
            elif kind == 'assign': text = child('target') + ' = ' + child('value')
            elif kind == 'return': text = 'return' + (' ' + child('value') if f['has_value'] == 'true' else '')
            elif kind == 'print': text = 'print ' + child('value') + (f" in [{f['unit']}]" if f['unit'] != '1' else '')
            elif kind == 'expression': text = child('value')
            elif kind in ('if', 'while'): text = kind + ' ' + child('condition') + ':'
            elif kind == 'for': text = f"for {f['index']} in range({child('start')}, {child('end')}):"
            else: raise ValueError(f'Unsupported statement: {kind}')
            lines.append('    ' * indent + text)
            if kind in ('for', 'while'): block(path + '/body', indent + 1)
            if kind == 'if':
                block(path + '/then', indent + 1)
                if path + '/else/0' in records:
                    lines.append('    ' * indent + 'else:')
                    block(path + '/else', indent + 1)
    block('body')
    return '\n'.join(lines)


def fence(code):
    delimiter = '`' * max(3, 1 + max((len(x) for x in re.findall(r'`+', code)), default=0))
    return f'{delimiter}text\n{code}\n{delimiter}'


def render(pack, compiler):
    def invoke(*args, request=None):
        return subprocess.run([str(compiler), 'env', *args], input=json.dumps(request) if request else None,
                              capture_output=True, text=True, check=True, timeout=60).stdout
    revision = next(b['revision'] for b in json.loads(invoke('branches', str(pack)))['branches'] if b['name'] == 'main')
    ids = sorted(line for line in invoke('graph', str(pack), revision).splitlines() if line.startswith('@'))
    views = [json.loads(invoke('inspect', str(pack), name, revision))['result'] for name in ids]
    anchors = {name: f'function-{i+1}' for i, name in enumerate(ids)}
    def links(names):
        return ', '.join(f'[{name}](#{anchors[name]})' if name in anchors else f'`{name}`' for name in names) or 'None'
    lines = [f'# {pack.stem} — readable program', '', f'Generated from [{pack.name}]({pack.name}). **Do not edit this companion by hand.**', '',
             'This is a compiler-derived pseudocode view of the stored program, not an executable source file or a correctness certificate. '
             'Numeric types, declared units, mutation and branch structure are retained. Unmarked integers use i64; decimals and short math calls use f64. Other literal types are shown. `range(start, end)` excludes the end. '
             'Long strings may be explicitly omitted by the compiler query. Internal revisions and hashes are intentionally not displayed.', '',
             'Build the compiler with `cargo build`, then regenerate from the repository root: `conda run -n base python scripts/render_packs.py`.', '',
             '## Functions', '', '| Function | Calls |', '|---|---|']
    lines += [f"| {links([v['id']])} | {links(v['calls'])} |" for v in views]
    for v in views:
        request = dict(schema='vibe.query.v1', snapshot=revision, op='nodes', id=v['id'], limit=256)
        records = {}
        while True:
            page = invoke('query', str(pack), request=request).splitlines()
            header = fields(page[0])
            if header['snapshot'] != revision or header['status'] != 'known': raise ValueError('Unresolved snapshot or nodes')
            for line in page[1:]:
                if line.startswith('body/'):
                    path, kind, *rest = line.split(' ', 2)
                    records[path] = (kind, fields(rest[0]) if rest else {})
            if header['complete'] == 'true': break
            request['cursor'] = header['cursor']
        params = ', '.join(p['name'] + ': ' + p['type'] for p in v['parameters'])
        namespace = v['id'].rsplit('.', 1)[0] + '.' if '.' in v['id'] else ''
        code = f"function {v['id']}({params}) -> {v['result']}:\n" + '\n'.join('    ' + line for line in pseudocode(records, namespace).splitlines())
        lines += ['', f'<a id="{anchors[v["id"]]}"></a>', f'### {v["id"]}', '',
                  f"Called by: {links(v['called_by'])}. Calls: {links(v['calls'])}.", '',
                  '<details>', '<summary>View implementation</summary>', '', fence(code), '', '</details>']
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT/'target/debug/vibec')
    parser.add_argument('--check', action='store_true', help='Fail if checked-in companions differ; do not write')
    args = parser.parse_args()
    packs = sorted((ROOT/'examples').rglob('*.vibepack'))
    outputs = {p.with_suffix('.vibepack.md'): render(p, args.compiler.resolve()) for p in packs}
    index = ['# Browse Vibe programs', '', 'These generated pages expose the actual stored functions, signatures and call relationships. The binary packs remain authoritative.', '']
    index += [f'- [{p.stem}]({p.with_suffix(".vibepack.md").relative_to(ROOT/"examples").as_posix()})' for p in packs]
    index += ['', 'Build the compiler first: `cargo build`.', 'Regenerate: `conda run -n base python scripts/render_packs.py`.', 'Check freshness without writing: `conda run -n base python scripts/render_packs.py --check`.', '']
    outputs[ROOT/'examples/PACKS.md'] = '\n'.join(index)
    stale = []
    for path, text in outputs.items():
        if args.check:
            if not path.exists() or path.read_text() != text: stale.append(str(path.relative_to(ROOT)))
        else: path.write_text(text)
    if stale: raise SystemExit('Stale or missing: ' + ', '.join(stale))
    print(f'{"Checked" if args.check else "Rendered"} {len(packs)} packs.')


if __name__ == '__main__': main()
