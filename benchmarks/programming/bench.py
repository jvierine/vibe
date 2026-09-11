"""Paired programming-efficiency benchmark. HDF5 evidence, no model API dependency."""
import argparse
import contextlib
import ctypes as ct
import fcntl
import hashlib
import itertools
import json
import os
from pathlib import Path
import platform
import shlex
import signal
import subprocess
import sys
import time
import uuid

import h5py
import numpy as np
from tasks import TASKS, cases, python_source, rust_source, vibe_body

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
COMPILER=ROOT/"target/debug/vibec"
LANGUAGES=("vibe","python","rust")
sys.path.insert(0,str(ROOT/"scripts"))
from native_build import compile_shared


def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()


@contextlib.contextmanager
def state(trial,mode="r+"):
    with (trial/"state.lock").open("a+b") as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        with h5py.File(trial/"trial.h5",mode) as data: yield data


def bounded_run(command,timeout,**kwargs):
    proc=subprocess.Popen(command,start_new_session=True,**kwargs)
    try: return proc.communicate(timeout=timeout),proc.returncode,False
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid,signal.SIGKILL)
        output=proc.communicate()
        return output,proc.returncode,True


def install(work,language,task,solution=False):
    if language=="python": (work/"solution.py").write_text(python_source(task,solution))
    elif language=="rust": (work/"solution.rs").write_text(rust_source(task,solution))
    else:
        author=("from pathlib import Path\nimport sys\n"
                f"sys.path.insert(0,{str(ROOT/'scripts')!r})\n"
                "from semantic_builder import Program,scalar,array,math,length,as_f64\n"
                "p=Program()\nb=p.function('@bench.solve',[('data',array())],scalar())\ndata,=b.args\n"
                +vibe_body(task,solution)+
                f"p.apply(Path({str(COMPILER)!r}),Path(__file__).parent/'program.vibepack')\n")
        (work/"author.py").write_text(author)
        subprocess.run([sys.executable,str(work/"author.py")],check=True,capture_output=True)


def prepare(args):
    trial=args.root.resolve()/f"{args.task}-{args.language}-{uuid.uuid4().hex[:12]}"
    work=trial/"workspace"; work.mkdir(parents=True)
    install(work,args.language,args.task)
    with h5py.File(trial/"trial.h5","x") as data:
        data.attrs.update(schema="vibe.programming-benchmark.v1",language=args.language,task=args.task,
            seed=args.seed,model=args.model,cohort=args.cohort,status="prepared",managed=False,
            seconds_budget=args.seconds,token_budget=args.tokens,created_utc_ns=time.time_ns(),
            platform=platform.platform(),python=sys.version,compiler_sha256=sha(COMPILER),
            harness_sha256=sha(__file__),tasks_sha256=sha(HERE/"tasks.py"),
            builder_sha256=sha(ROOT/"scripts/semantic_builder.py"),native_cache_sha256=sha(ROOT/"scripts/native_build.py"))
        data.attrs["rustc"]=subprocess.check_output(["rustc","--version"],text=True).strip()
        data.attrs["git_head"]=subprocess.check_output(["git","-C",str(ROOT),"rev-parse","HEAD"],text=True).strip()
        data.attrs["git_diff_sha256"]=hashlib.sha256(subprocess.check_output(["git","-C",str(ROOT),"diff","HEAD"])).hexdigest()
        for split,count,seed in [("public",3,args.seed),("heldout",64,args.seed+1000003)]:
            inputs,expected=cases(args.task,seed,count)
            group=data.create_group(split); group.create_dataset("expected",data=expected)
            for i,x in enumerate(inputs): group.create_dataset(f"inputs/{i}",data=np.asarray(x,dtype=np.float64))
        data.create_group("checks")
    check=shlex.join([sys.executable,str(Path(__file__).resolve()),"check",str(trial)])
    prompt=f"""# Programming benchmark: {args.task}

Language: {args.language}. Category: {TASKS[args.task][0]}.

{TASKS[args.task][1]}

Implement solve(data) -> float64 in the provided workspace. Python gets a list
of floats; Rust gets &[f64]; Vibe gets Array<f64,1> at @bench.solve.
Use the language's standard library and the supplied Vibe semantic builder/tooling.
Do not use network/package installation, other language implementations, subprocess
numerical backends, or the controller's tasks.py/reference/heldout data. Do not edit
the benchmark controller or test records. The harness is not a security sandbox.
For Vibe, use typed environment transactions; author.py is an optional IR builder.
Do not edit compiler-generated C or bootstrap .vibe text. No function may mutate
the input. Inputs satisfy the contract; no general malformed-input API is required.

Time budget: {args.seconds} seconds; input+output token budget: {args.tokens}.
All task-directed reasoning, documentation reads, code generation and tool calls
count toward development cost. Work until done or the budget expires.

Public check command:
{check}

Public checks provide feedback; heldout evaluation happens once after the driver
exits. Do not call finish yourself during a managed run. Functional tolerances are
rtol=2e-12 and atol=2e-12 (stable_norm uses atol=0 to test tiny values).
"""
    (work/"TASK.md").write_text(prompt)
    if args.language=="vibe":
        (work/"TOOLING.md").write_text(f"Compiler: {COMPILER}\nBuilder: {ROOT/'scripts/semantic_builder.py'}\nAPI documentation: {ROOT/'AGENT_API.md'}\nLanguage: {ROOT/'LANGUAGE.md'}\nRun author.py with the base conda Python.\n")
    # Public examples may be read by agents; no CSV or serialized giant ASTs.
    inputs,expected=cases(args.task,args.seed,3)
    with h5py.File(work/"public_examples.h5","w") as data:
        data.create_dataset("expected",data=expected)
        for i,x in enumerate(inputs): data.create_dataset(f"inputs/{i}",data=np.asarray(x))
    return trial


def start(trial,managed=False):
    with state(trial) as data:
        if data.attrs["status"]!="prepared": raise ValueError("trial already started")
        data.attrs.update(status="active",managed=managed,start_monotonic_ns=time.monotonic_ns(),start_utc_ns=time.time_ns())


class Array(ct.Structure):
    _fields_=[("data",ct.POINTER(ct.c_double)),("length",ct.c_size_t)]


def worker(args):
    """Runs candidate code in a killable subprocess, NOT a security boundary."""
    work=args.trial/"workspace"; build=args.trial/"build"; build.mkdir(exist_ok=True)
    with h5py.File(args.input,"r") as data:
        inputs=[data[f"inputs/{i}"][:] for i in range(len(data["expected"]))]
        language=data.attrs["language"]
    begin=time.perf_counter(); hit=False
    if language=="python":
        # Compile the exact current bytes: timestamp/size-based .pyc reuse can
        # otherwise execute the previous candidate after a same-size fast edit.
        source=work/"solution.py"; namespace={"__name__":"candidate","__file__":str(source)}
        exec(compile(source.read_bytes(),str(source),"exec"),namespace)
        def solve(x):
            values=x.tolist(); result=namespace["solve"](values)
            if values!=x.tolist(): raise ValueError("candidate mutated input")
            return result
    elif language=="vibe":
        pack=work/"program.vibepack"
        abi=json.loads(subprocess.check_output([str(COMPILER),"env","abi",str(pack),"@bench.solve"]))
        if len(abi["parameters"])!=1 or abi["parameters"][0]["type"]!={"kind":"array","scalar":"f64","rank":1,"mutable":False,"si_dimensions":{},"layout":"pointer_then_size_t","extent":"dynamic"} or abi["result"]!={"kind":"scalar","scalar":"f64","si_dimensions":{}}:
            raise ValueError("candidate ABI must be immutable Array<f64,1> -> f64, dimensionless")
        source=subprocess.check_output([str(COMPILER),"env","emit-c",str(pack),"--at",abi["revision"]])
        artifact=compile_shared(source,build/"cache"); hit=artifact.cache_hit
        lib=ct.CDLL(str(artifact.path)); raw=getattr(lib,abi["symbol"])
        raw.argtypes=[Array]; raw.restype=ct.c_double
        solve=lambda x:raw(Array(x.ctypes.data_as(ct.POINTER(ct.c_double)),x.size))
    else:
        source=(work/"solution.rs").read_text()+'''\n#[unsafe(no_mangle)]
pub unsafe extern "C" fn bench_entry(data:*const f64, len:usize)->f64 {
    solve(unsafe {std::slice::from_raw_parts(data,len)})
}\n'''
        version=subprocess.check_output(["rustc","--version","--verbose"])
        key=hashlib.sha256(source.encode()+version+platform.platform().encode()+b"edition2024-O-cdylib").hexdigest()
        directory=build/key; directory.mkdir(exist_ok=True)
        cpath=directory/"wrapper.rs"; libpath=directory/("module.dylib" if sys.platform=="darwin" else "module.so")
        stamp=directory/"artifact.sha256"
        hit=libpath.exists() and stamp.exists() and stamp.read_text()==sha(libpath)
        if not hit:
            cpath.write_text(source)
            subprocess.run(["rustc","--edition=2024","-O","--crate-type=cdylib",str(cpath),"-o",str(libpath)],check=True)
            stamp.write_text(sha(libpath))
        lib=ct.CDLL(str(libpath)); raw=lib.bench_entry
        raw.argtypes=[ct.POINTER(ct.c_double),ct.c_size_t]; raw.restype=ct.c_double
        solve=lambda x:raw(x.ctypes.data_as(ct.POINTER(ct.c_double)),x.size)
    build_seconds=time.perf_counter()-begin; begin=time.perf_counter()
    original=[x.copy() for x in inputs]
    actual=np.asarray([solve(x) for x in inputs],dtype=np.float64)
    for x,y in zip(inputs,original): np.testing.assert_array_equal(x,y,err_msg="candidate mutated input")
    with h5py.File(args.output,"w") as data:
        data.create_dataset("actual",data=actual)
        data.attrs.update(load_or_build_seconds=build_seconds,evaluation_seconds=time.perf_counter()-begin,cache_hit=hit)


def fingerprint(trial,language):
    name={"vibe":"program.vibepack","python":"solution.py","rust":"solution.rs"}[language]
    path=trial/"workspace"/name
    return sha(path) if path.exists() else "missing"


def evaluate(trial,final=False):
    with state(trial) as data:
        if data.attrs["status"]!="active": raise ValueError("start trial before checking; finished trials are immutable")
        split="heldout" if final else "public"; index=len(data["checks"])
        group=data["checks"].create_group(str(index)); group.attrs["split"]=split
        language=data.attrs["language"]; group.attrs["candidate_sha256"]=fingerprint(trial,language)
        input_path=trial/f"check-{index}-input.h5"; output_path=trial/f"check-{index}-output.h5"
        with h5py.File(input_path,"w") as inp:
            inp.attrs["language"]=language
            data.copy(data[split]["inputs"],inp,"inputs"); data.copy(data[split]["expected"],inp,"expected")
        log=trial/f"check-{index}.log"; begin=time.perf_counter()
        elapsed=(time.monotonic_ns()-data.attrs["start_monotonic_ns"])/1e9
        remaining=float(data.attrs["seconds_budget"])-elapsed
        # Final scoring has its own fixed verifier allowance. It is excluded
        # from agent development time, but retains a timeout/failure outcome.
        timeout=30 if final else min(30,max(.001,remaining))
        with log.open("wb") as output:
            _,code,timed_out=bounded_run([sys.executable,str(Path(__file__).resolve()),"_worker",str(trial),"--input",str(input_path),"--output",str(output_path)],timeout,stdout=output,stderr=output)
        passed=False; passed_cases=0; expected=data[split]["expected"][:]
        if code==0 and output_path.exists():
            with h5py.File(output_path) as output:
                actual=output["actual"][:]
                if actual.shape==expected.shape:
                    atol=0 if data.attrs["task"]=="stable_norm" else 2e-12
                    good=np.isclose(actual,expected,rtol=2e-12,atol=atol,equal_nan=False)
                    passed_cases=int(np.sum(good)); passed=bool(np.all(good))
                    group.create_dataset("actual",data=actual)
                for key,value in output.attrs.items(): group.attrs[key]=value
        group.attrs.update(passed=passed,passed_cases=passed_cases,total_cases=len(expected),exit_code=code,
            timed_out=timed_out,check_seconds=time.perf_counter()-begin,
            elapsed_seconds=(time.monotonic_ns()-data.attrs["start_monotonic_ns"])/1e9)
        if fingerprint(trial,language)!=group.attrs["candidate_sha256"]:
            passed=False; group.attrs.update(passed=False,candidate_changed_during_check=True)
        if passed and not final and "first_public_pass_seconds" not in data.attrs:
            data.attrs["first_public_pass_seconds"]=group.attrs["elapsed_seconds"]
            if "usage" in data:
                data.attrs["first_public_pass_token_checkpoint"]=int(data["usage"].attrs["input_tokens"]+data["usage"].attrs["output_tokens"])
        result=dict(passed=passed,passed_cases=passed_cases,total_cases=len(expected),timed_out=timed_out,log=str(log))
        if final:
            usage=data.get("usage")
            known=usage is not None and bool(usage.attrs["complete"])
            data.attrs["observed_tokens"]=int(usage.attrs["input_tokens"]+usage.attrs["output_tokens"]) if usage is not None else -1
            tokens=int(usage.attrs["input_tokens"]+usage.attrs["output_tokens"]) if known else -1
            eligible=bool(data.attrs["managed"] and known and data.attrs["cohort"]=="agent")
            in_budget=known and data.attrs["development_seconds"]<=data.attrs["seconds_budget"] and tokens<=data.attrs["token_budget"]
            data.attrs.update(status="finished",functional_pass=passed,within_budget=in_budget,
                              measured_success=passed and in_budget and not data.attrs.get("driver_failed",False),
                              comparison_eligible=eligible,tokens=tokens,token_telemetry_complete=known)
        return result


def finish(trial):
    with state(trial) as data:
        if data.attrs["status"]!="active": raise ValueError("trial is not active")
        data.attrs["development_seconds"]=(time.monotonic_ns()-data.attrs["start_monotonic_ns"])/1e9
    return evaluate(trial,True)


def usage(trial):
    record=json.load(sys.stdin)
    record.setdefault("complete",False)
    if set(record)!={"input_tokens","output_tokens","tool_calls","source","complete"}: raise ValueError("usage requires input_tokens, output_tokens, tool_calls, source; optional complete")
    if type(record["complete"]) is not bool: raise ValueError("complete must be boolean")
    if any(type(record[k]) is not int or record[k]<0 for k in ("input_tokens","output_tokens","tool_calls")): raise ValueError("usage counters must be nonnegative integers")
    if not isinstance(record["source"],str) or not 0<len(record["source"])<=512: raise ValueError("name telemetry source")
    with state(trial) as data:
        if data.attrs["status"]!="active": raise ValueError("usage is recorded before finish")
        if "usage" in data:
            previous=dict(data["usage"].attrs)
            if previous==record: return
            if previous["complete"]: raise ValueError("final usage already submitted")
            if previous["source"]!=record["source"] or any(record[k]<previous[k] for k in ("input_tokens","output_tokens","tool_calls")):
                raise ValueError("usage snapshots must be cumulative, monotonic and from the same source")
        snapshots=data.require_group("usage_snapshots")
        snapshots.create_group(str(len(snapshots))).attrs.update(record)
        data.require_group("usage").attrs.update(record)


def run(trial,driver):
    if driver and driver[0]=="--": driver=driver[1:]
    if not driver: raise ValueError("supply a driver command after --")
    start(trial,True)
    with state(trial) as data: timeout=float(data.attrs["seconds_budget"])
    env=os.environ.copy(); env.update(BENCH_TRIAL=str(trial),BENCH_TASK=str(trial/"workspace/TASK.md"),BENCH_PYTHON=sys.executable,BENCH_HARNESS=str(Path(__file__).resolve()))
    with (trial/"driver.log").open("wb") as log:
        try:
            _,code,timed_out=bounded_run(driver,timeout,cwd=trial/"workspace",env=env,stdout=log,stderr=log)
        except OSError as error:
            log.write(str(error).encode()); code=127; timed_out=False
    with state(trial) as data: data.attrs.update(driver_exit_code=code,driver_timed_out=timed_out,driver_failed=code!=0,driver_command=shlex.join(driver))
    return finish(trial)


def summary(root):
    rows=[]
    for path in sorted(root.rglob("trial.h5")):
        with h5py.File(path) as data:
            if data.attrs["status"]!="finished": continue
            row=dict(data.attrs); row["path"]=str(path)
            row["failed_public_checks"]=sum(not bool(g.attrs["passed"]) for g in data["checks"].values() if g.attrs["split"]=="public")
            rows.append(row)
    print("# Programming token-efficiency results\n")
    print(f"Finished trials: {len(rows)}. Eligible managed agent trials with runner telemetry: {sum(r['comparison_eligible'] for r in rows)}.\n")
    print("Primary cost is total input + output tokens, including reasoning/output tokens and repeated context as reported by the runner. Include cached input in input_tokens; do not double-count cached or reasoning subcategories. Calibration/manual runs are excluded. Missing telemetry is unknown, not zero.\n")
    managed=[r for r in rows if r["managed"] and r["cohort"]=="agent"]
    for model in sorted(set(r["model"] for r in managed)):
        print(f"## Model: {model}\n")
        print("| Language | Attempts | Functional passes | Missing telemetry | Budget-qualified passes | Total tokens / qualified pass, including failures |\n|---|---:|---:|---:|---:|---:|")
        for lang in LANGUAGES:
            subset=[r for r in managed if r["language"]==lang and r["model"]==model]
            missing=sum(not r["comparison_eligible"] for r in subset)
            successes=sum(r["measured_success"] and r["comparison_eligible"] for r in subset)
            cost=f"{sum(r['tokens'] for r in subset)/successes:.1f}" if successes and not missing else "unknown" if missing or not subset else "no successes"
            print(f"| {lang} | {len(subset)} | {sum(r['functional_pass'] for r in subset)} | {missing} | {successes} | {cost} |")
    print("\nPaired comparisons match task, seed, model and budgets. A comparator/Vibe token ratio >1 favors Vibe. Both-success ratios are conditional: always consider failures and missing telemetry alongside them.")
    for other in ("python","rust"):
        pairs=[]
        for r in rows:
            if not r["comparison_eligible"] or r["language"]!="vibe": continue
            matches=[s for s in rows if s["comparison_eligible"] and s["language"]==other and all(s[k]==r[k] for k in ("task","seed","model","seconds_budget","token_budget"))]
            if len(matches)==1 and sum(all(s[k]==r[k] for k in ("task","seed","model","seconds_budget","token_budget")) and s["language"]=="vibe" and s["comparison_eligible"] for s in rows)==1: pairs.append((r,matches[0]))
        success=[b["tokens"]/a["tokens"] for a,b in pairs if a["measured_success"] and b["measured_success"] and a["tokens"]>0]
        ratio=f"{np.median(success):.3f}" if success else "not available"
        print(f"\nVibe vs {other}: {len(pairs)} fully instrumented matched pairs; Vibe successes {sum(a['measured_success'] for a,b in pairs)}, {other} successes {sum(b['measured_success'] for a,b in pairs)}; both-pass pairs {len(success)}, median {other}/Vibe token ratio {ratio}.")
    print("\nNo causal language-efficiency claim is justified by calibration runs or this small task suite alone.")


def main():
    parser=argparse.ArgumentParser(description=__doc__); sub=parser.add_subparsers(dest="command",required=True)
    p=sub.add_parser("prepare"); p.add_argument("--root",type=Path,default=ROOT/"target/programming-benchmark")
    p.add_argument("--language",choices=LANGUAGES,required=True); p.add_argument("--task",choices=TASKS,required=True)
    p.add_argument("--seed",type=int,default=1); p.add_argument("--model",required=True); p.add_argument("--cohort",choices=("agent","calibration"),default="agent")
    p.add_argument("--seconds",type=float,default=600); p.add_argument("--tokens",type=int,default=64000)
    for command in ("start","check","finish","usage","run","_worker"):
        p=sub.add_parser(command); p.add_argument("trial",type=Path)
        if command=="run": p.add_argument("driver",nargs=argparse.REMAINDER)
        if command=="_worker": p.add_argument("--input",type=Path); p.add_argument("--output",type=Path)
    p=sub.add_parser("summary"); p.add_argument("root",type=Path)
    p=sub.add_parser("self-test"); p.add_argument("--root",type=Path,default=ROOT/"target/programming-calibration")
    p=sub.add_parser("plan"); p.add_argument("--output",type=Path,required=True); p.add_argument("--seed",type=int,default=20260911); p.add_argument("--repetitions",type=int,default=6)
    args=parser.parse_args()
    if hasattr(args,"trial"): args.trial=args.trial.resolve()
    if args.command=="prepare":
        if not np.isfinite(args.seconds) or args.seconds<=0 or args.tokens<=0: parser.error("budgets must be finite and positive")
        print(prepare(args))
    elif args.command=="start": start(args.trial)
    elif args.command=="check": print(json.dumps(evaluate(args.trial)))
    elif args.command=="finish": print(json.dumps(finish(args.trial)))
    elif args.command=="usage": usage(args.trial)
    elif args.command=="run": print(json.dumps(run(args.trial,args.driver)))
    elif args.command=="_worker": worker(args)
    elif args.command=="summary": summary(args.root)
    elif args.command=="plan":
        if args.repetitions<1: parser.error("repetitions must be positive")
        rng=np.random.default_rng(args.seed); rows=[]; permutations=list(itertools.permutations(LANGUAGES))
        for rep in range(args.repetitions):
            for task in rng.permutation(list(TASKS)):
                instance=int(rng.integers(0,2**31))
                for lang in permutations[rep%6]: rows.append((str(task),lang,str(instance),str(rep)))
        args.output.parent.mkdir(parents=True,exist_ok=True)
        with h5py.File(args.output,"x") as data:
            data.attrs.update(schema="vibe.programming-plan.v1",seed=args.seed,columns="task;language;instance_seed;repetition")
            data.create_dataset("trials",data=np.asarray(rows,dtype=h5py.string_dtype()))
        print(f"{len(rows)} paired trials in {args.output}; run each in a fresh agent context")
    else:
        for task in TASKS:
            for lang in LANGUAGES:
                setup=argparse.Namespace(root=args.root,task=task,language=lang,seed=19,model="reference-fixture",cohort="calibration",seconds=600,tokens=64000)
                trial=prepare(setup); start(trial)
                assert not evaluate(trial)["passed"],(task,lang,"starter unexpectedly passes")
                install(trial/"workspace",lang,task,True)
                assert evaluate(trial)["passed"],(task,lang,"reference public failure",trial)
                assert finish(trial)["passed"],(task,lang,"reference heldout failure",trial)
                print(f"PASS calibration {task}/{lang}",flush=True)
        summary(args.root)


if __name__=="__main__": main()
