"""Host ABI/data boundary for the compiled Vibe metablate program (no dynamics).

SPDX-License-Identifier: GPL-3.0-or-later
"""
import ctypes as ct
import json
from pathlib import Path
import subprocess
import numpy as np
from native_build import compile_shared

ROOT = Path(__file__).resolve().parents[1]
PACK = ROOT / "examples/metablate/metablate.vibepack"
COMPILER = ROOT / "target/debug/vibec"


class Array(ct.Structure):
    _fields_ = [("data", ct.POINTER(ct.c_double)), ("len", ct.c_size_t)]


def descriptor(a):
    if not isinstance(a,np.ndarray) or a.dtype != np.float64 or not a.flags.c_contiguous:
        raise TypeError("native Vibe buffer must be contiguous numpy float64")
    return Array(a.ctypes.data_as(ct.POINTER(ct.c_double)),a.size)


class Native:
    def __init__(self, build=ROOT/"target/metablate", sanitize=False):
        build = Path(build)
        build.mkdir(parents=True,exist_ok=True)
        # Generated C is disposable compiler output, never authored model code.
        heads = json.loads(subprocess.check_output([str(COMPILER),"env","branches",str(PACK)],text=True))
        self.revision = next(b["revision"] for b in heads["branches"] if b["name"]=="main")
        source = subprocess.check_output([str(COMPILER),"env","emit-c",str(PACK),"--at",self.revision])
        artifact=compile_shared(source,build/"cache",sanitize=sanitize)
        self.source_hash=artifact.source_hash
        self.compile_command=artifact.command
        self.cache_hit=artifact.cache_hit
        self.cache_key=artifact.key
        self.lib = ct.CDLL(str(artifact.path))
        self.functions = {}

    def function(self,name):
        if name not in self.functions:
            identity = "@metablate."+name
            info = json.loads(subprocess.check_output([str(COMPILER),"env","abi",str(PACK),identity,self.revision],text=True))
            if self.revision is not None and self.revision != info["revision"]:
                raise RuntimeError("semantic pack changed while binding ABI")
            self.revision = info["revision"]
            signature = info
            def typ(s):
                if s["kind"]=="array" and s["scalar"]=="f64" and s["rank"]==1: return Array
                if s["kind"]=="none": return None
                if s["kind"]=="scalar" and s["scalar"]=="f64": return ct.c_double
                if s["kind"]=="scalar" and s["scalar"]=="i64": return ct.c_int64
                raise TypeError(f"unsupported ABI type {s}")
            raw = getattr(self.lib,signature["symbol"])
            raw.argtypes = [typ(item["type"]) for item in signature["parameters"]]
            raw.restype = typ(signature["result"])
            mutable = [item["type"].get("mutable",False) for item in signature["parameters"]]
            def checked_call(*args):
                if len(args)!=len(raw.argtypes): raise TypeError("incorrect argument count")
                for i,(a,t,m) in enumerate(zip(args,raw.argtypes,mutable)):
                    if t is Array and m:
                        if not a.flags.writeable: raise ValueError("mutable Vibe parameter requires writable memory")
                        for j,other in enumerate(args):
                            if j!=i and isinstance(other,np.ndarray) and np.shares_memory(a,other):
                                raise ValueError("overlapping native buffers are not supported")
                return raw(*(descriptor(a) if t is Array else a for a,t in zip(args,raw.argtypes)))
            self.functions[name] = checked_call
        return self.functions[name]

    def config(self,material,settings,rtol=1e-7,max_step=.05):
        if rtol<=0 or max_step<=0: raise ValueError("positive integration tolerances required")
        cfg = np.zeros(28,dtype=np.float64)
        cfg[:11] = [material["rho_m"],material["c"],material["L"],material["mu"],material["CA"],material["CB"],
                    settings.heat_transfer_coefficient,settings.drag_coefficient,settings.emissivity,1.21,settings.initial_temperature_k]
        cfg[15] = settings.diameter_um*1e-6
        cfg[11] = self.function("particle_mass")(cfg[15],cfg[0])
        cfg[12] = max(cfg[11]*1e-6,1e-18)
        cfg[13:15] = max_step,rtol
        cfg[16:21] = settings.latitude_deg,settings.longitude_deg,settings.start_altitude_km*1e3,0,90
        cfg[24:28] = [1e-10,1e-5,1e-3,1e-5]
        return cfg

    def simulate(self,cfg,logrho,speed,zenith,record=True):
        self.validate(cfg,logrho)
        if speed<=0 or not 0<=zenith<=90: raise ValueError("invalid initial speed/zenith")
        trajectory = np.empty(6*50000 if record else 1,dtype=np.float64)
        summary = np.zeros(10,dtype=np.float64)
        n = self.function("integrate")(cfg,logrho,speed,zenith,trajectory,summary)
        if n<0: raise RuntimeError(f"Vibe integration failed: status={n}, speed={speed}, zenith={zenith}")
        if not np.all(np.isfinite(summary)): raise RuntimeError("nonfinite Vibe trajectory summary")
        return (trajectory[:6*n].reshape(-1,6).copy() if record else None),summary

    def boundary(self,cfg,logrho,zenith,target=1350,mode=0,index=-1,speed=0,lower=5000,upper=25000,xtol=.05):
        self.validate(cfg,logrho)
        if lower>=upper or xtol<=0 or mode not in (0,1) or index not in (-1,0,6,7,8):
            raise ValueError("invalid boundary search")
        local_cfg = cfg.copy()
        summary=np.zeros(10,dtype=np.float64)
        root = self.function("boundary")(local_cfg,logrho,zenith,target,mode,index,speed,lower,upper,xtol,summary)
        if root<0: raise RuntimeError(f"Vibe boundary search failed: {root}, summary={summary}")
        return root,summary

    @staticmethod
    def validate(cfg,logrho):
        if cfg.shape!=(28,) or logrho.shape!=(801,): raise ValueError("expected 28 configuration values and 801 atmosphere samples")
        if not np.all(np.isfinite(cfg)) or not np.all(np.isfinite(logrho)): raise ValueError("nonfinite input")
        if min(cfg[0],cfg[1],cfg[3],cfg[10],cfg[11],cfg[12],cfg[13],cfg[14],cfg[20],*cfg[24:28])<=0: raise ValueError("nonpositive physical/integration parameter")
        if cfg[12]>=cfg[11] or min(cfg[6],cfg[7])<0 or not 0<=cfg[8]<=1:
            raise ValueError("invalid mass floor or thermal/drag parameters")
