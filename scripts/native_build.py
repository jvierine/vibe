"""Content-addressed native artifact cache for compiler-generated standalone C.

The cache separates sanitizer/toolchain/options and serializes identical builds.
No handwritten model code or persisted JSON manifests are involved.
"""
from dataclasses import dataclass
import fcntl
import hashlib
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess


@dataclass(frozen=True)
class Artifact:
    path: Path
    key: str
    source_hash: str
    cache_hit: bool
    command: str


def compile_shared(source, cache, sanitize=False):
    cache=Path(cache)
    compiler=shutil.which("clang")
    if compiler is None: raise RuntimeError("Clang is required for native compilation")
    flags=["-std=c17","-O2","-ffp-contract=off","-fPIC",
           "-dynamiclib" if platform.system()=="Darwin" else "-shared"]
    if sanitize: flags.extend(["-fsanitize=undefined","-fno-sanitize-recover=all"])
    version=subprocess.check_output([compiler,"--version"])
    # Standard platform/toolchain inputs; arbitrary user include dependencies
    # are outside this standalone compiler-output cache's contract.
    environment=[os.environ.get(name,"") for name in
                 ("SDKROOT","DEVELOPER_DIR","MACOSX_DEPLOYMENT_TARGET","CPATH","C_INCLUDE_PATH","LIBRARY_PATH")]
    inputs=[b"vibe.native-cache.v1",source,os.fsencode(Path(compiler).resolve()),version,
            hashlib.sha256(Path(compiler).read_bytes()).digest(),platform.platform().encode(),
            repr(flags).encode(),repr(environment).encode()]
    digest=hashlib.sha256()
    for part in inputs: digest.update(len(part).to_bytes(8,"big")); digest.update(part)
    key=digest.hexdigest()
    directory=cache/key
    directory.mkdir(parents=True,exist_ok=True)
    output=directory/("module.dylib" if platform.system()=="Darwin" else "module.so")
    pending=directory/("pending"+output.suffix)
    cpath=directory/"module.c"
    stamp=directory/"artifact.sha256"
    command=[compiler,*flags,str(cpath),"-lm","-o",str(pending)]
    with (directory/"build.lock").open("a+b") as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        hit=(output.is_file() and stamp.is_file() and
             stamp.read_text().strip()==hashlib.sha256(output.read_bytes()).hexdigest())
        if not hit:
            cpath.write_bytes(source)  # Disposable compiler output.
            subprocess.run(command,check=True)
            os.replace(pending,output)
            temporary=directory/"pending.sha256"
            temporary.write_text(hashlib.sha256(output.read_bytes()).hexdigest()+"\n")
            os.replace(temporary,stamp)
    return Artifact(output,key,hashlib.sha256(source).hexdigest(),hit,shlex.join(command))
