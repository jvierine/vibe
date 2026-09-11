"""Reproduce ablatemm studies with compiled Vibe computation and Python plots.

SPDX-License-Identifier: GPL-3.0-or-later
The Python reference is used only for validation. Trajectories, adaptive steps,
events, boundary roots, mass/temperature summaries, geometric tangency, escape
speed, and radiation-pressure/PR calculations in the delivered studies are Vibe.
"""
import argparse
from dataclasses import replace
import hashlib
import importlib.metadata
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import shutil
import time

import h5py
import numpy as np
from scipy.integrate import solve_ivp
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.figure import Figure
import xarray as xr

from metablate_native import Native, ROOT, PACK, COMPILER

FIELDS = ("t","mass","velocity","position","temperature","altitude")
SUMMARY = ("maximum_temperature_k","retained_mass_fraction","peak_height_m","end_time_s",
           "end_height_m","termination_code","accepted_steps","rejected_steps","initial_speed_m_s","initial_mass_kg")


def digest(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Study:
    def __init__(self,args):
        self.args=args
        sys.path.insert(0,str(args.study_root))
        import model_sio as base
        import model_sio_survival as survival
        import model_sio_escape_angle as escape
        import memo001_nonmelt_profiles as memo
        import radiation_pressure_pr as radiation
        self.base,self.survival,self.escape,self.memo,self.radiation=base,survival,escape,memo,radiation
        self.settings=base.RunSettings()
        self.metablate=base.import_metablate(args.metablate)
        base.install_stefan_boltzmann_fix(self.metablate)
        self.material=base.sio_material()
        self.native=Native(sanitize=args.sanitize)
        self.atmosphere=base.FixedMSISProfile(self.metablate,self.settings)
        # NRLMSIS returns float32. Widen the supplied log-density samples exactly;
        # interpolation and exponentiation of this fixed table execute in Vibe.
        self.logrho=np.ascontiguousarray(self.atmosphere.log_total_density,dtype=np.float64)
        self.output=args.output
        self.output.mkdir(parents=True,exist_ok=True)
        self.h5=h5py.File(self.output/"metablate_vibe.h5","w")
        self.h5.attrs.update(generating_script=str(Path(__file__).resolve()),
                            computations="compiled Vibe semantic program; no Python ODE/optimizer callbacks",
                            vibe_revision=self.native.revision,
                            generated_c_sha256=self.native.source_hash,
                            compiler_command=self.native.compile_command,
                            native_cache_hit=self.native.cache_hit,native_cache_key=self.native.cache_key,
                            atmosphere="NRLMSIS 2.1 fixed 801-sample profile, 250 m spacing; external input data",
                            corrections="Stefan-Boltzmann radiation; local zenith; ground event uses WGS84 altitude",
                            model_caveat="legacy straight ray plus scalar gravity, not an orbital capture or melt-fraction model",
                            rtol=1e-7,max_step_s=.05,
                            tolerance_policy="empirical numerical validation, not rigorous error bounds",
                            summary_columns=";".join(SUMMARY))
        prov=self.h5.create_group("provenance")
        for folder in [args.study_root,args.metablate/"src/metablate",ROOT/"scripts",ROOT/"src"]:
            for path in sorted(folder.rglob("*.py" if folder!=ROOT/"src" else "*.rs")):
                if folder==ROOT/"scripts" and path.name not in ["author_metablate.py","semantic_builder.py","metablate_native.py","native_build.py","run_metablate.py"]: continue
                prov.attrs[str(path)]=digest(path)
        prov.attrs["metablate_commit"]=subprocess.check_output(["git","-C",str(args.metablate),"rev-parse","HEAD"],text=True).strip()
        prov.attrs["vibec_commit"]=subprocess.check_output(["git","-C",str(ROOT),"rev-parse","HEAD"],text=True).strip()
        prov.attrs["vibec_worktree_status"]=subprocess.check_output(["git","-C",str(ROOT),"status","--short"],text=True)
        prov.attrs["pack_sha256"]=digest(PACK)
        for package in ["numpy","scipy","pymsis","h5py","matplotlib"]:
            prov.attrs[package]=importlib.metadata.version(package)
        ag=self.h5.create_group("atmosphere")
        ag.create_dataset("altitude_m",data=self.atmosphere.altitude_m)
        ag.create_dataset("log_total_density",data=self.logrho)
        for key,val in vars(self.settings).items(): ag.attrs[key]=val
        self.case_count=0
        self.roots={}
        self.validation=[]
        self.start=time.perf_counter()
        self.install_plot_provenance()

    def install_plot_provenance(self):
        self.old_savefig=Figure.savefig
        old=self.old_savefig
        def savefig(fig,path,**kwargs):
            if not self.args.hide_provenance:
                fig.canvas.draw()
                fig.set_layout_engine(None)
                bounds=fig.get_tightbbox(fig.canvas.get_renderer())
                provenance_y=(bounds.y1+.15)/fig.get_figheight()
                fig.text(.5,provenance_y,"Computation: Vibe · Plot: run_metablate.py",ha="center",va="bottom",fontsize=10)
            old(fig,path,**dict(kwargs,bbox_inches="tight"))
            old(fig,Path(path).with_suffix(".svg"),bbox_inches="tight")
        Figure.savefig=savefig

    def config(self,diameter=100,**changes):
        settings=replace(self.settings,diameter_um=diameter,**changes)
        return self.native.config(self.material,settings)

    def track(self,group,cfg,speed,zenith,record=True):
        trajectory,summary=self.native.simulate(cfg,self.logrho,float(speed),float(zenith),record)
        self.case_count+=1
        assert 0<summary[1]<=1.00000001, (speed,zenith,summary)
        assert summary[0]>0
        if group is not None:
            g=self.h5.require_group(group)
            g.create_dataset("config",data=cfg)
            g.attrs["zenith_deg"]=zenith
            for key,val in zip(SUMMARY,summary): g.attrs[key]=val
            if record:
                assert np.all(np.diff(trajectory[:,0])>0)
                for j,name in enumerate(FIELDS): g.create_dataset(name,data=trajectory[:,j],compression="gzip",shuffle=True)
        dataset=None if trajectory is None else xr.Dataset({name:(["t"],trajectory[:,j]) for j,name in enumerate(FIELDS) if name!="t"},coords={"t":trajectory[:,0]})
        return dataset,summary

    def root(self,cfg,zenith,target=1350,mode=0,**kwargs):
        key=(cfg.tobytes(),zenith,target,mode,tuple(sorted(kwargs.items())))
        if key not in self.roots:
            self.roots[key]=self.native.boundary(cfg,self.logrho,zenith,target=target,mode=mode,**kwargs)
        result=self.roots[key]
        if mode==0: assert abs(result[1][0]-target)<1.0,result
        if mode==1: assert abs(result[1][1]-target)<5e-5,result
        return result

    def check(self,name,observed,reference,rtol=1e-9,atol=0):
        observed,reference=np.asarray(observed),np.asarray(reference)
        error=float(np.max(np.abs(observed-reference)))
        np.testing.assert_allclose(observed,reference,rtol=rtol,atol=atol,err_msg=name)
        self.validation.append((name,error,rtol,atol))

    def validate_kernels(self):
        n=self.native; mat=self.material; m=self.metablate
        out=np.zeros(3)
        for lat,lon,h in [(0,0,0),(90,0,180000),(-90,20,1000),(69.5866,19.227,180000),(-35,151,85000)]:
            n.function("geodetic_to_ecef")(lat,lon,h,out)
            self.check("geodetic_to_ecef",out,m.coordinates.geodetic2ecef(lat,lon,h),atol=2e-8)
            xyz=out.copy(); n.function("ecef_to_geodetic")(*xyz,out)
            self.check("ecef_to_geodetic",out,m.coordinates.ecef2geodetic(*xyz),atol=2e-7)
        rng=np.random.default_rng(20260911)
        for _ in range(40):
            mass=10**rng.uniform(-12,-6); temp=rng.uniform(300,2800); vel=rng.uniform(5000,40000); air=10**rng.uniform(-10,-4)
            dm=n.function("thermal_mass_loss")(mass,temp,mat["rho_m"],mat["mu"],mat["CA"],mat["CB"],1.21)
            ref=m.physics.thermal_ablation.thermal_ablation_hill_et_al_2005(mass,temp,mat,1.21)
            self.check("thermal_mass_loss",dm,ref,rtol=2e-12,atol=1e-40)
            dt=n.function("temperature_rate")(mass,vel,temp,mat["rho_m"],mat["c"],mat["L"],1.21,air,-dm,1,290,.9)
            ref=m.physics.thermal_ablation.temperature_rate_hill_et_al_2005(mass,vel,temp,mat,1.21,air,-ref,1,290,.9)
            self.check("temperature_rate",dt,ref,rtol=2e-12,atol=1e-7)
        for h in np.linspace(-1000,201000,101):
            rho=n.function("atmosphere_density")(self.logrho,float(h))
            ref=float(self.atmosphere.density(None,None,None,h)["Total"].values)
            self.check("atmosphere_interpolation",rho,ref,rtol=3e-14)
        cfg=self.config()
        self.check("tangent_zenith",n.function("tangent_zenith")(cfg),self.memo.tangent_zenith(m,self.settings),atol=1e-9)
        self.check("escape_speed",n.function("escape_speed")(cfg),self.escape.local_escape_speed(m,self.settings),atol=1e-9)
        thin=np.full(801,-100.0)
        ground,summary=n.simulate(cfg,thin,11000,0)
        assert summary[5]==3 and abs(ground[-1,5])<1e-5,summary
        self.check("ground_event_altitude",ground[-1,5],0.,atol=1e-5)
        _,summary=n.simulate(cfg,self.logrho,60000,0)
        assert summary[5]==2,summary
        self.check("low_mass_event",summary[1],cfg[12]/cfg[11],atol=1e-10)
        small=np.zeros(6); summary=np.zeros(10)
        assert n.function("integrate")(cfg,self.logrho,10000,0,small,summary)==-3
        with tempfile.TemporaryDirectory(prefix="vibe-metablate-contract-") as folder:
            project=Path(folder)/"test.vibepack"; shutil.copyfile(PACK,project)
            inspection=json.loads(subprocess.check_output([str(COMPILER),"env","inspect",str(project),"@metablate.thermal_mass_loss_si"],text=True))
            transaction=dict(schema="vibe.transaction.v0",name="reject dimensionless mass",branch="main",base_revision=inspection["revision"],
                reads=[dict(id="@metablate.thermal_mass_loss_si",revision=inspection["object_revision"])],
                operations=[dict(op="put_function",expected_revision=None,object=dict(id="@validation.bad_units",parameters=[],result=dict(kind="scalar",scalar="f64",unit="kg/s"),
                    body=[dict(kind="return",value=dict(kind="call",function="@metablate.thermal_mass_loss_si",arguments=[dict(kind="number",scalar="f64",value="1") for _ in range(7)]))]))])
            before=digest(project)
            rejected=subprocess.run([str(COMPILER),"env","apply",str(project)],input=json.dumps(transaction),text=True,capture_output=True)
            assert rejected.returncode and "argument 1" in rejected.stderr,rejected.stderr
            assert before==digest(project)
        print(f"PASS {len(self.validation)} kernel/coordinate/atmosphere checks",flush=True)

    def reference_trajectory(self,cfg,speed,zenith):
        """Independent SciPy driver of original metablate RHS; validation only."""
        m=self.metablate
        settings=replace(self.settings,diameter_um=cfg[15]*1e6,emissivity=cfg[8],heat_transfer_coefficient=cfg[6],drag_coefficient=cfg[7])
        material=dict(self.material,rho_m=cfg[0])
        model=self.base.build_model(m,self.atmosphere,settings,cfg[11])
        origin=m.coordinates.geodetic2ecef(cfg[16],cfg[17],cfg[18])
        direction=m.coordinates.enu2ecef(cfg[16],cfg[17],cfg[18],*(-m.coordinates.azel_to_cart(0.,90-zenith,1.)).tolist())
        def rhs(t,y): return model.rhs(t,y[0],y[1:],material,cfg[6],cfg[7],np.datetime64(settings.epoch),origin,direction)
        def cool(t,y): return y[3]-400
        def low(t,y): return y[0]-cfg[12]
        def ground(t,y): return m.coordinates.ecef2geodetic(*(origin-direction*y[2]))[2]
        for event in [cool,low,ground]: event.terminal=True; event.direction=-1
        return solve_ivp(rhs,(0,cfg[20]),[cfg[11],speed,0,cfg[10]],rtol=cfg[14],
                         atol=[cfg[24]*cfg[11],cfg[25],cfg[26],cfg[27]],max_step=cfg[13],
                         events=(cool,low,ground),dense_output=True)

    def validate_trajectories(self):
        cases=[(100,6734.0,0,.9),(100,11041.,60,.9),(50,16000.,30,.9),
               (100,25000.,0,.9),(50,11041.,89,.9),(100,16000.,76.5,.9),
               # Largest discrepancies against the stored, loose-tolerance studies.
               (20,25500.,0,.9),(20,19500.,60,.9),(100,11041.188914855948,0,.1)]
        cg=self.h5.create_group("validation/trajectories")
        for i,(diameter,speed,zenith,emissivity) in enumerate(cases):
            cfg=self.config(diameter,emissivity=emissivity)
            tr,s=self.native.simulate(cfg,self.logrho,speed,zenith)
            ref=self.reference_trajectory(cfg,speed,zenith)
            assert ref.success,ref.message
            points=tr[tr[:,0]<=ref.t[-1]]
            reference=ref.sol(points[:,0]).T
            self.check("trajectory_temperature",points[:,4],reference[:,3],rtol=0,atol=.1)
            self.check("trajectory_mass_fraction",points[:,1]/cfg[11],reference[:,0]/cfg[11],rtol=0,atol=2e-5)
            self.check("trajectory_velocity",points[:,2],reference[:,1],rtol=2e-5,atol=.02)
            self.check("trajectory_position",points[:,3],reference[:,2],rtol=0,atol=2)
            self.check("trajectory_event_time",tr[-1,0],ref.t[-1],rtol=0,atol=.002)
            tighter=cfg.copy(); tighter[14]=1e-9; tighter[13]=.0125; tighter[24:28]*=.01
            _,tight=self.native.simulate(tighter,self.logrho,speed,zenith)
            self.check("refined_peak_temperature",s[0],tight[0],rtol=0,atol=.15)
            self.check("refined_retained_fraction",s[1],tight[1],rtol=0,atol=2e-5)
            g=cg.create_group(str(i)); g.attrs.update(diameter_um=diameter,speed_m_s=speed,zenith_deg=zenith,emissivity=emissivity)
            g.create_dataset("vibe",data=tr,compression="gzip")
            g.create_dataset("reference_at_vibe_times",data=reference,compression="gzip")
            g.create_dataset("comparison_times",data=points[:,0])
            g.create_dataset("summary",data=s); g.create_dataset("refined_summary",data=tight)
            print(f"PASS independent trajectory d={diameter:g} um v={speed/1000:g} km/s z={zenith:g}; peak refinement {abs(s[0]-tight[0]):.4g} K",flush=True)
        self.h5.flush()

    def nonmelt(self):
        rows=[]
        tangent=self.native.function("tangent_zenith")(self.config())
        factors=(1.,1.025,1.05,1.10,1.15,1.20)
        for diameter in (100.,50.):
            cfg=self.config(diameter); results=[]
            for zenith in (0.,30.,60.,tangent):
                speed,_=self.root(cfg,zenith)
                comparisons=[]
                for factor in factors:
                    track,summary=self.track(f"nonmelt/diameter_{diameter:g}/zenith_{zenith:.9f}/factor_{factor:.3f}",cfg,speed*factor,zenith)
                    if factor>1: assert summary[0]>1350
                    comparisons.append((factor,speed*factor,track))
                g=self.h5[f"nonmelt/diameter_{diameter:g}/zenith_{zenith:.9f}"]
                g.attrs["threshold_speed_m_s"]=speed
                results.append((zenith,speed,comparisons[0][2],comparisons))
                print(f"nonmelt d={diameter:g} um z={zenith:.6f} v={speed/1000:.6f} km/s",flush=True)
            settings=replace(self.settings,diameter_um=diameter)
            if diameter==100:
                self.base.make_plot(self.output/"sio_100um_1350K.png",settings,cfg[11],results[:3])
            rows.append((settings,cfg[11],[results[j] for j in (0,1,3)]))
        self.memo.plot(rows,tangent,self.output/"memo001_nonmelt_profiles.png")
        self.h5["nonmelt"].attrs["tangent_zenith_deg"]=tangent
        self.h5.flush()

    def survival_study(self):
        cfg=self.config(); results=[]; comparison_results=[]
        for zenith in (0.,30.,60.):
            solutions=[]
            for fraction in (.01,.1,.5):
                speed,summary=self.root(cfg,zenith,target=fraction,mode=1,lower=3000)
                assert summary[0]<3000,summary
                track,_=self.track(f"survival/boundaries/zenith_{zenith:g}/retained_{fraction:g}",cfg,speed,zenith)
                solutions.append((fraction,speed,track))
                print(f"survival z={zenith:g} retained={fraction:g} v={speed/1000:.6f} km/s",flush=True)
            results.append((zenith,solutions))
            speed=solutions[0][1]; tracks=[]
            for factor in (1.,1.02,1.05):
                track,_=self.track(f"survival/comparisons/zenith_{zenith:g}/factor_{factor:g}",cfg,speed*factor,zenith)
                tracks.append((factor,speed*factor,track))
            comparison_results.append((zenith,tracks))
        self.survival.make_summary_plot(self.output/"sio_survival_profiles.png",replace(self.settings,threshold_k=3000),cfg[11],comparison_results)
        self.survival.make_boundary_plot(self.output/"sio_survival_thresholds.png",results)
        diameters=np.array([10.,20.,50.,100.,200.,500.,1000.]); speeds=np.linspace(6000,30000,17); zeniths=np.array([0.,30.,60.])
        retained=np.empty((3,7,17)); temperatures=np.empty_like(retained); masses=np.empty(7)
        for j,diameter in enumerate(diameters):
            cfg=self.config(diameter); masses[j]=cfg[11]
            for i,zenith in enumerate(zeniths):
                for k,speed in enumerate(speeds):
                    _,summary=self.track(None,cfg,speed,zenith,False)
                    retained[i,j,k]=summary[1]; temperatures[i,j,k]=summary[0]
            print(f"mass/velocity sweep complete: {diameter:g} um",flush=True)
        sweep=dict(zenith_deg=zeniths,diameter_um=diameters,initial_mass_kg=masses,speed_m_s=speeds,
                   retained_mass_fraction=retained,maximum_temperature_k=temperatures)
        g=self.h5.create_group("survival/mass_velocity_sweep")
        for name,data in sweep.items(): g.create_dataset(name,data=data)
        # Display floor only: dense event localization can land a few ulps below
        # 1e-6. Keep raw values in HDF5, avoid blank contour holes at that boundary.
        plot_sweep=dict(sweep,retained_mass_fraction=np.clip(retained,1e-6,1.0))
        self.survival.make_mass_velocity_plot(self.output/"sio_survival_mass_velocity_sweep.png",plot_sweep)
        self.h5.flush()

    def escape_study(self):
        speed=self.native.function("escape_speed")(self.config())
        results=[]
        for diameter in (50.,100.):
            cfg=self.config(diameter); tracks=[]
            for zenith in (0.,30.,60.,70.,75.,80.,85.,87.,89.):
                track,_=self.track(f"escape/diameter_{diameter:g}/zenith_{zenith:g}",cfg,speed,zenith)
                tracks.append((zenith,track))
            results.append((diameter,cfg[11],tracks))
        self.escape.make_plot(self.output/"sio_escape_speed_angle_size.png",self.settings,speed,results)
        self.h5.flush()
        print("escape-angle/size sweep complete",flush=True)

    def sensitivity(self):
        speed=self.native.function("escape_speed")(self.config())
        specs=[("heat_transfer_coefficient",6,np.linspace(.05,1,20),r"Heat transfer $\Lambda$"),
               ("drag_coefficient",7,np.linspace(.5,5,19),r"Drag $\Gamma$"),
               ("emissivity",8,np.linspace(.1,1,19),r"Emissivity $\epsilon$"),
               ("density",0,np.linspace(100,2190,22),r"Density (kg m$^{-3}$)")]
        fig,axes=plt.subplots(2,2,figsize=(7.06,6.1),constrained_layout=True)
        for ax,(name,index,values,label) in zip(axes.flat,specs):
            temperatures=[]; retained=[]
            for v in values:
                cfg=self.config()
                cfg[index]=v
                if index==0:
                    cfg[11]=self.native.function("particle_mass")(cfg[15],v); cfg[12]=max(cfg[11]*1e-6,1e-18)
                _,summary=self.track(None,cfg,speed,0,False)
                temperatures.append(summary[0]); retained.append(summary[1])
            g=self.h5.create_group("sensitivity/"+name)
            for key,data in [("value",values),("maximum_temperature_k",temperatures),("retained_mass_fraction",retained)]: g.create_dataset(key,data=data)
            ax.plot(values,temperatures,color="#b2182b"); ax.axhline(1350,color="black",ls=":")
            massax=ax.twinx(); massax.plot(values,np.array(retained)*100,color="#2166ac",ls="--")
            ax.set(xlabel=label,ylabel="Peak temperature (K)"); massax.set(ylabel="Mass retained (%)",ylim=(0,105)); ax.grid(alpha=.2)
        for name,index,lo,hi in [("heat_transfer",6,.05,1),("drag",7,1,8),("density",0,100,2190)]:
            root,_=self.root(self.config(),0,index=index,speed=speed,lower=lo,upper=hi,xtol=1e-6)
            self.h5["sensitivity"].attrs["critical_"+name]=root
            print(f"sensitivity critical {name}={root:.8g}",flush=True)
        fig.suptitle(f"100 µm SiO at {speed/1000:.3f} km/s, vertical entry")
        fig.savefig(self.output/"sio_parameter_sensitivity.png",dpi=220); plt.close(fig)
        self.h5.flush()

    def radiation_study(self):
        radii=np.geomspace(.01e-6,1000e-6,1201)
        data=np.empty((len(radii),4)); out=np.zeros(4)
        for i,r in enumerate(radii):
            self.native.function("radiation_pressure")(r,2190,1,149597870700.,out)
            data[i]=out
        self.check("radiation_beta",data[:,0],self.radiation.beta_for_radius(radii,2190,1),rtol=1e-14)
        self.check("radiation_pr_time",data[:,1],self.radiation.pr_inspiral_time(radii,149597870700.,2190,1),rtol=1e-14)
        g=self.h5.create_group("radiation")
        for name,values in [("radius_m",radii),("beta",data[:,0]),("pr_inspiral_time_s",data[:,1])]: g.create_dataset(name,data=values)
        g.attrs["blowout_radius_m"]=data[0,2]; g.attrs["force_balance_radius_m"]=data[0,3]
        self.radiation.make_plot(self.output/"sio_radiation_pressure_1au.png",radii*2e6,data[:,0],data[:,1]/(365.25*86400),data[0,2]*2e6)
        self.h5.flush()

    def historical_comparison(self):
        """Report legacy default-tolerance differences, not exact-equality claims."""
        hg=self.h5.create_group("validation/historical")
        def compare(name,new,old,path):
            g=hg.create_group(name); delta=np.asarray(new)-np.asarray(old)
            g.create_dataset("vibe",data=new); g.create_dataset("legacy",data=old); g.create_dataset("difference",data=delta)
            g.attrs["reference_file"]=str(path); g.attrs["reference_sha256"]=digest(path)
            g.attrs["max_absolute_difference"]=np.max(np.abs(delta))
            print(f"legacy comparison {name}: max absolute difference={np.max(np.abs(delta)):.6g}",flush=True)
        path=self.args.study_root/"output/sio_100um_1350K.h5"
        if path.exists():
            with h5py.File(path) as old:
                compare("nonmelt_speed_m_s",[self.root(self.config(),z)[0] for z in (0.,30.,60.)],
                        [old[f"zenith_{z:g}_deg"].attrs["threshold_speed_m_s"] for z in (0.,30.,60.)],path)
        path=self.args.study_root/"output_survival/sio_100um_survival_boundaries.h5"
        if path.exists():
            with h5py.File(path) as old:
                compare("survival_boundary_speed_m_s",[self.root(self.config(),z,target=f,mode=1,lower=3000)[0] for z in (0.,30.,60.) for f in (.01,.1,.5)],
                        [old[f"zenith_{z:g}_deg/retained_{100*f:g}_percent"].attrs["threshold_speed_m_s"] for z in (0.,30.,60.) for f in (.01,.1,.5)],path)
                for field in ["retained_mass_fraction","maximum_temperature_k"]:
                    compare("mass_velocity_"+field,self.h5["survival/mass_velocity_sweep/"+field][:],old["mass_velocity_sweep/"+field][:],path)
        for diameter in (50.,100.):
            path=self.args.study_root/f"output/memo001_nonmelt_{diameter:g}um.h5"
            if path.exists():
                with h5py.File(path) as old:
                    groups=sorted([k for k in old if k.startswith("zenith_")],key=lambda k:float(k.split("_")[1]))
                    angles=[0.,30.,float(old.attrs["geometric_max_zenith_deg"])]
                    compare(f"memo001_{diameter:g}um_speed_m_s",[self.root(self.config(diameter),z)[0] for z in angles],
                            [old[k].attrs["threshold_speed_m_s"] for k in groups],path)
        path=self.args.study_root/"output/sio_escape_speed_angle_size.h5"
        if path.exists():
            with h5py.File(path) as old:
                for diameter in (50.,100.):
                    angles=(0.,30.,60.,70.,75.,80.,85.,87.,89.)
                    for field,legacy_field in [("maximum_temperature_k","maximum_temperature_k"),("retained_mass_fraction","final_mass_fraction")]:
                        compare(f"escape_{diameter:g}um_{field}",[self.h5[f"escape/diameter_{diameter:g}/zenith_{z:g}"].attrs[field] for z in angles],
                                [old[f"diameter_{diameter:g}_um/zenith_{z:g}_deg"].attrs[legacy_field] for z in angles],path)
        path=self.args.study_root/"output/sio_100um_parameter_sensitivity.h5"
        if path.exists():
            with h5py.File(path) as old:
                for name in old:
                    compare("sensitivity_"+name,self.h5["sensitivity/"+name+"/maximum_temperature_k"][:],old[name+"/maximum_temperature_k"][:],path)

    def finish(self):
        self.h5.attrs["trajectory_cases"]=self.case_count
        self.h5.attrs["boundary_searches"]=len(self.roots)
        self.h5.attrs["validation_checks"]=len(self.validation)
        self.h5.attrs["elapsed_seconds"]=time.perf_counter()-self.start
        vg=self.h5.require_group("validation")
        vg.create_dataset("check_names",data=np.array([x[0] for x in self.validation],dtype=h5py.string_dtype()))
        vg.create_dataset("check_error_rtol_atol",data=[x[1:] for x in self.validation])
        self.h5.attrs["status"]="passed"
        self.h5.close(); Figure.savefig=self.old_savefig
        print(f"PASS: {self.case_count} study trajectories, {len(self.roots)} Vibe boundary searches, {len(self.validation)} numerical checks",flush=True)
        print(f"Data and figures: {self.output}",flush=True)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--study-root",type=Path,default=Path("/Users/jvi019/src/ablatemm"))
    parser.add_argument("--metablate",type=Path,default=Path("/Users/jvi019/src/ablate"))
    parser.add_argument("--output",type=Path,default=Path("/Users/jvi019/src/ablatemm/output_vibe"))
    parser.add_argument("--validate-only",action="store_true")
    parser.add_argument("--sanitize",action="store_true")
    parser.add_argument("--hide-provenance",action="store_true")
    args=parser.parse_args()
    study=Study(args)
    try:
        study.validate_kernels(); study.validate_trajectories()
        if not args.validate_only:
            study.nonmelt(); study.survival_study(); study.escape_study(); study.sensitivity(); study.radiation_study(); study.historical_comparison()
        study.finish()
    except BaseException as error:
        study.h5.attrs["status"]="failed"; study.h5.attrs["failure"]=repr(error); study.h5.close()
        Figure.savefig=study.old_savefig
        raise


if __name__=="__main__": main()
