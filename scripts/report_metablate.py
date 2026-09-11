"""Plot stored Vibe results at LaTeX size and benchmark the native/reference solvers.

No new physical model is evaluated in Python except the independent reference.
SPDX-License-Identifier: GPL-3.0-or-later
"""
import argparse
from pathlib import Path
import platform
import subprocess
import time
import hashlib

import h5py
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.colors import LogNorm

from run_metablate import Study, Figure
from metablate_native import ROOT


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data",type=Path,default=Path("/Users/jvi019/src/ablatemm/output_vibe/metablate_vibe.h5"))
    parser.add_argument("--output",type=Path,default=ROOT/"output/pdf/metablate")
    parser.add_argument("--repeats",type=int,default=3)
    args=parser.parse_args()
    if args.repeats<1: parser.error("repeats must be positive")
    args.output.mkdir(parents=True,exist_ok=True)
    plt.rcParams.update({"font.size":11,"axes.labelsize":11,"axes.titlesize":11,
                         "xtick.labelsize":10,"ytick.labelsize":10,"legend.fontsize":10})
    with h5py.File(args.data,"r") as data:
        assert data.attrs["status"]=="passed"
        # Actual memo text width is 7.06 inches. Use a fixed canvas; never shrink
        # a much larger source figure into the report.
        def save(fig,name):
            fig.savefig(args.output/(name+".png"),dpi=240)
            fig.savefig(args.output/(name+".svg"))
            plt.close(fig)
        fig,axes=plt.subplots(1,2,figsize=(7.06,2.8),layout="constrained")
        colors=["#0072b2","#d55e00","#009e73","#cc79a7"]
        for diameter,ax in zip((100,50),axes):
            groups=data[f"nonmelt/diameter_{diameter}"]
            for color,key in zip(colors,sorted(groups,key=lambda s:float(s.split("_")[1]))):
                g=groups[key]; t=g["factor_1.000"]
                z=float(key.split("_")[1])
                ax.plot(t["temperature"][:],t["altitude"][:]/1000,color=color,label=f"{z:.1f}°")
            ax.axvline(1350,color=".5",ls=":")
            ax.set(xlabel="Temperature (K)",ylabel="Altitude (km)",ylim=(60,150),title=f"{diameter} µm SiO")
            ax.grid(alpha=.2)
        axes[0].legend(title="Initial zenith",loc="upper right",frameon=False)
        save(fig,"report_profiles")

        fig,axes=plt.subplots(2,1,figsize=(7.06,3.3),layout="constrained")
        g=data["validation/trajectories/6"]
        time_values=g["comparison_times"][:]
        native=g["vibe"][:len(time_values)]
        ref=g["reference_at_vibe_times"][:]
        axes[0].plot(time_values,native[:,4],color="#0072b2",label="Vibe")
        axes[0].plot(time_values,ref[:,3],color="#d55e00",ls="--",label="Original RHS + SciPy")
        axes[0].set(ylabel="Temperature (K)")
        axes[0].legend(loc="upper left",frameon=False,ncol=2)
        axes[1].plot(time_values,(native[:,4]-ref[:,3])*1000,color="#0072b2")
        axes[1].set(xlabel="Time (s)",ylabel="Difference (mK)")
        for ax in axes: ax.grid(alpha=.2)
        save(fig,"report_validation")

        fig,axes=plt.subplots(1,3,figsize=(7.06,3.15),layout="constrained",sharey=True)
        g=data["survival/mass_velocity_sweep"]
        for i,ax in enumerate(axes):
            mesh=ax.pcolormesh(g["speed_m_s"][:]/1000,g["diameter_um"][:],
                              np.clip(g["retained_mass_fraction"][i],1e-6,1),
                              shading="nearest",norm=LogNorm(1e-6,1),cmap="viridis")
            ax.set(yscale="log",xlabel="Speed (km/s)",title=f"Zenith {g['zenith_deg'][i]:g}°",xlim=(6,30),ylim=(10,1000))
        axes[0].set_ylabel("Initial diameter (µm)")
        fig.colorbar(mesh,ax=axes,label="Retained mass fraction",ticks=[1e-6,1e-4,1e-2,1],shrink=.9,pad=.02)
        save(fig,"report_survival")

        # Build outside timing; use precisely the same source RHS and tolerance
        # mapping as the independent numerical validation.
        settings=argparse.Namespace(study_root=Path("/Users/jvi019/src/ablatemm"),
            metablate=Path("/Users/jvi019/src/ablate"),sanitize=False,
            output=ROOT/"target/metablate-report-benchmark",hide_provenance=True)
        study=Study(settings)
        study.h5.close(); Figure.savefig=study.old_savefig
        cases=[]
        for key in sorted(data["validation/trajectories"],key=int):
            a=data["validation/trajectories/"+key].attrs
            cases.append((study.config(a["diameter_um"],emissivity=a["emissivity"]),float(a["speed_m_s"]),float(a["zenith_deg"])))
        def native_batch():
            for cfg,speed,zenith in cases: study.native.simulate(cfg,study.logrho,speed,zenith)
        def reference_batch():
            for cfg,speed,zenith in cases:
                result=study.reference_trajectory(cfg,speed,zenith)
                assert result.success
        native_batch(); reference_batch()
        native_times=[]; reference_times=[]
        for repeat in range(args.repeats):
            jobs=[(native_batch,native_times),(reference_batch,reference_times)]
            if repeat%2: jobs.reverse()
            for job,times in jobs:
                start=time.perf_counter(); job(); times.append(time.perf_counter()-start)
        ratio=float(np.median(reference_times)/np.median(native_times))
        out=args.output/"report_evidence.h5"
        with h5py.File(out,"w") as evidence:
            evidence.attrs.update(source_data=str(args.data),source_sha256=hashlib.sha256(args.data.read_bytes()).hexdigest(),
                                  generating_script=str(Path(__file__).resolve()),
                                  script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                                  native_revision=study.native.revision,compiler_command=study.native.compile_command,
                                  platform=platform.platform(),machine=platform.machine(),python=platform.python_version(),
                                  cpu=subprocess.check_output(["sysctl","-n","machdep.cpu.brand_string"],text=True).strip(),
                                  clang=subprocess.check_output(["clang","--version"],text=True).splitlines()[0],
                                  cases_per_batch=len(cases),repetitions=args.repeats,
                                  median_ratio=ratio,
                                  timing_scope="warm serial integration batches; compilation, atmosphere, plotting and reference resampling excluded; native includes output copy and host validation")
            evidence.create_dataset("native_batch_seconds",data=native_times)
            evidence.create_dataset("reference_batch_seconds",data=reference_times)
            names=data["validation/check_names"].asstr()[:]
            values=data["validation/check_error_rtol_atol"][:]
            for name in sorted(set(names)):
                evidence.require_group("numerical_maxima").attrs[name]=np.max(values[names==name,0])
            evidence.attrs["study_elapsed_seconds"]=data.attrs["elapsed_seconds"]
        print("native batch seconds",native_times,flush=True)
        print("reference batch seconds",reference_times,flush=True)
        print(f"median native/reference ratio: {ratio:.2f}x",flush=True)
        print(out,flush=True)


if __name__=="__main__": main()
