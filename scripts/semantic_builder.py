"""Small in-memory builder for typed Vibe transactions; never writes source/JSON files.

Python constructs semantic objects only. Expressions are never numerically
evaluated here. The compiler checks and executes the resulting program.
"""
from contextlib import contextmanager
import json
import subprocess


def scalar(name="f64", unit=None):
    return dict(kind="scalar", scalar=name, **({"unit": unit} if unit else {}))


def array(mutable=False, element="f64"):
    return dict(kind="array", element=element, rank=1, mutable=mutable)


class E:
    def __init__(self, node, typ="f64"):
        self.node, self.typ = node, typ

    def __bool__(self):
        raise TypeError("a Vibe expression is not a Python condition")

    def binary(self, op, rhs):
        rhs = value(rhs, self.typ)
        return E(dict(kind="binary", operator=op, left=self.node, right=rhs.node), self.typ)

    def compare(self, op, rhs):
        rhs = value(rhs, self.typ)
        return E(dict(kind="compare", operator=op, left=self.node, right=rhs.node), "bool")

    def __add__(self, rhs): return self.binary("+", rhs)
    def __sub__(self, rhs): return self.binary("-", rhs)
    def __mul__(self, rhs): return self.binary("*", rhs)
    def __truediv__(self, rhs): return self.binary("/", rhs)
    def __radd__(self, lhs): return value(lhs, self.typ) + self
    def __rsub__(self, lhs): return value(lhs, self.typ) - self
    def __rmul__(self, lhs): return value(lhs, self.typ) * self
    def __rtruediv__(self, lhs): return value(lhs, self.typ) / self
    def __neg__(self): return E(dict(kind="unary", operator="-", value=self.node), self.typ)
    def __lt__(self, rhs): return self.compare("<", rhs)
    def __le__(self, rhs): return self.compare("<=", rhs)
    def __gt__(self, rhs): return self.compare(">", rhs)
    def __ge__(self, rhs): return self.compare(">=", rhs)
    def __eq__(self, rhs): return self.compare("==", rhs)
    def __ne__(self, rhs): return self.compare("!=", rhs)
    def __getitem__(self, index):
        return E(dict(kind="index", array=self.node, index=value(index, "i64").node), self.typ.split(":")[-1])


def value(x, typ="f64", unit=None):
    if isinstance(x, E):
        return x
    return E(dict(kind="number", value=str(x), scalar=typ,
                  **({"unit": unit} if unit else {})), typ)


def call(name, *args, result="f64"):
    return E(dict(kind="call", function=name, arguments=[value(x).node for x in args]), result)


def math(name, *args):
    return call("@math.f64." + name, *args, result="bool" if name == "isfinite" else "f64")


def as_i64(x): return call("@cast.i64_from_f64", x, result="i64")
def as_f64(x): return call("@cast.f64_from_i64", x)
def length(x): return call("len", x, result="i64")


class Function:
    def __init__(self, name, params, result=None):
        self.object = dict(id=name, parameters=[dict(name=k, type=t) for k, t in params],
                           result=result or dict(kind="none"), body=[])
        self.body = self.object["body"]
        self.args = [E(dict(kind="variable", name=k),
                       "array:" + t["element"] if t["kind"] == "array" else t["scalar"])
                     for k, t in params]

    def let(self, name, expr, mutable=False, typ=None):
        expr = value(expr)
        self.body.append(dict(kind="let", name=name, mutable=mutable, value=expr.node,
                              **({"annotation": typ} if typ else {})))
        return E(dict(kind="variable", name=name), expr.typ)

    def zeros(self, name, count):
        return self.let(name, E(dict(kind="array", values=[value(0.0).node for _ in range(count)]), "array:f64"), True)

    def assign(self, target, expr):
        self.body.append(dict(kind="assign", target=target.node, value=value(expr, target.typ).node))

    def invoke(self, name, *args):
        self.body.append(dict(kind="expression", value=call(name, *args).node))

    def ret(self, expr=None):
        self.body.append(dict(kind="return", **({"value": value(expr).node} if expr is not None else {})))

    @contextmanager
    def block(self, node, field):
        self.body.append(node)
        outer = self.body
        self.body = node[field]
        try: yield
        finally: self.body = outer

    @contextmanager
    def if_(self, condition):
        with self.block(dict(kind="if", condition=condition.node, then_body=[], else_body=[]), "then_body"):
            yield

    @contextmanager
    def while_(self, condition):
        with self.block(dict(kind="while", condition=condition.node, body=[]), "body"):
            yield

    @contextmanager
    def loop(self, name, end, start=0):
        node = dict(kind="for", index=name, start=value(start, "i64").node,
                    end=value(end, "i64").node, body=[])
        with self.block(node, "body"):
            yield E(dict(kind="variable", name=name), "i64")


class Program:
    def __init__(self): self.functions = []

    def function(self, name, params, result=None):
        f = Function(name, params, result)
        self.functions.append(f.object)
        return f

    def apply(self, compiler, pack):
        # Re-authoring uses existing revisions, preserving immutable history.
        old = {}
        base = None
        if pack.exists():
            result = json.loads(subprocess.check_output([str(compiler), "env", "branches", str(pack)], text=True))
            base = next(b["revision"] for b in result["branches"] if b["name"] == "main")
            for obj in self.functions:
                inspected = subprocess.run([str(compiler), "env", "inspect", str(pack), obj["id"]], text=True, capture_output=True)
                if inspected.returncode == 0:
                    old[obj["id"]] = json.loads(inspected.stdout)["object_revision"]
        transaction = dict(schema="vibe.transaction.v0", name="author metablate scientific package",
                           branch="main", base_revision=base, reads=[], operations=[
                               dict(op="put_function", expected_revision=old.get(obj["id"]), object=obj)
                               for obj in self.functions])
        proc = subprocess.run([str(compiler), "env", "apply", str(pack)], input=json.dumps(transaction), text=True, capture_output=True)
        if proc.returncode:
            raise RuntimeError(proc.stderr)
        return json.loads(proc.stdout)
