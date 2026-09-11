"""Author the metablate Kero--Szasz study backend as typed Vibe semantic objects.

SPDX-License-Identifier: GPL-3.0-or-later
Physics/coordinates adapted from Daniel Kastinen and Johan Kero's metablate;
SiO study corrections from /Users/jvi019/src/ablatemm/model_sio.py.
See examples/metablate/LICENSE and README.md. Python builds IR, not trajectories.
"""
from pathlib import Path
import argparse
from scipy.integrate._ivp.rk import RK45
from scipy import constants
from semantic_builder import Program, scalar, array, value, call, math, as_i64, as_f64, length

ROOT = Path(__file__).resolve().parents[1]
PREFIX = "@metablate."
PI = 3.141592653589793
A_EARTH = 6378137.0
B_EARTH = 6356752.3142
ESQ = 0.00669437999014
E1SQ = 0.00673949674228
GM = 6.67430e-11 * 5.9742e24  # Preserve package's Earth mass, not the escape-speed GM.


def make_program():
    p = Program()

    def fn(name, params, result=None): return p.function(PREFIX + name, params, result)
    def c(name, *args, result="f64"): return call(PREFIX + name, *args, result=result)
    def fparams(*names): return [(name, scalar()) for name in names]
    def invoke(b, name, *args): b.invoke(PREFIX + name, *args)

    b = fn("particle_mass", [("diameter",scalar(unit="m")),("density",scalar(unit="kg/m^3"))],scalar(unit="kg"))
    diameter,rho = b.args
    b.ret(PI/6 * diameter*diameter*diameter*rho)

    b = fn("geodetic_to_ecef", fparams("latitude_deg","longitude_deg","altitude_m")+[("out",array(True))])
    lat,lon,h,out = b.args
    la = b.let("latitude",lat*PI/180)
    lo = b.let("longitude",lon*PI/180)
    xi = b.let("xi",math("sqrt",1-ESQ*math("sin",la)*math("sin",la)))
    b.assign(out[0],(A_EARTH/xi+h)*math("cos",la)*math("cos",lo))
    b.assign(out[1],(A_EARTH/xi+h)*math("cos",la)*math("sin",lo))
    b.assign(out[2],(A_EARTH/xi*(1-ESQ)+h)*math("sin",la))
    b.ret()

    b = fn("ecef_to_geodetic",fparams("x","y","z")+[("out",array(True))])
    x,y,z,out = b.args
    r = b.let("radius_xy",math("sqrt",x*x+y*y))
    with b.if_(r < 1e-9):
        b.assign(out[0],90.0)
        with b.if_(z<0): b.assign(out[0],-90.0)
        with b.if_(z==0): b.assign(out[0],0.0)
        b.assign(out[1],0.0)
        b.assign(out[2],math("abs",z)-B_EARTH)
        b.ret()
    eq = A_EARTH*A_EARTH-B_EARTH*B_EARTH
    ff = b.let("ff",54*B_EARTH*B_EARTH*z*z)
    gg = b.let("gg",r*r+(1-ESQ)*z*z-ESQ*eq)
    cc = b.let("cc",ESQ*ESQ*ff*r*r/(gg*gg*gg))
    ss = b.let("ss",math("cbrt",1+cc+math("sqrt",cc*cc+2*cc)))
    pp = b.let("pp",ff/(3*(ss+1/ss+1)*(ss+1/ss+1)*gg*gg))
    qq = b.let("qq",math("sqrt",1+2*ESQ*ESQ*pp))
    r0 = b.let("r0",-pp*ESQ*r/(1+qq)+math("sqrt",0.5*A_EARTH*A_EARTH*(1+1/qq)-pp*(1-ESQ)*z*z/(qq*(1+qq))-0.5*pp*r*r))
    uu = b.let("uu",math("sqrt",(r-ESQ*r0)*(r-ESQ*r0)+z*z))
    vv = b.let("vv",math("sqrt",(r-ESQ*r0)*(r-ESQ*r0)+(1-ESQ)*z*z))
    z0 = b.let("z0",B_EARTH*B_EARTH*z/(A_EARTH*vv))
    b.assign(out[0],math("atan",(z+E1SQ*z0)/r)*180/PI)
    b.assign(out[1],math("atan2",y,x)*180/PI)
    b.assign(out[2],uu*(1-B_EARTH*B_EARTH/(A_EARTH*vv)))
    b.ret()

    b = fn("geometry",[("cfg",array()),("zenith_deg",scalar()),("out",array(True))])
    cfg,zen,out = b.args
    xyz = b.zeros("xyz",3)
    invoke(b,"geodetic_to_ecef",cfg[16],cfg[17],cfg[18],xyz)
    la = b.let("lat",cfg[16]*PI/180)
    lo = b.let("lon",cfg[17]*PI/180)
    zz = b.let("zenith",zen*PI/180)
    north = b.let("north",-math("sin",zz))
    up = b.let("up",-math("cos",zz))
    with b.loop("i",3) as i: b.assign(out[i],xyz[i])
    b.assign(out[3],-math("sin",la)*math("cos",lo)*north+math("cos",la)*math("cos",lo)*up)
    b.assign(out[4],-math("sin",la)*math("sin",lo)*north+math("cos",la)*math("sin",lo)*up)
    b.assign(out[5],math("cos",la)*north+math("sin",la)*up)
    b.ret()

    b = fn("altitude",[("geometry",array()),("position",scalar())],scalar())
    geom,pos = b.args
    geo = b.zeros("geo",3)
    invoke(b,"ecef_to_geodetic",geom[0]-geom[3]*pos,geom[1]-geom[4]*pos,geom[2]-geom[5]*pos,geo)
    b.ret(geo[2])

    b = fn("atmosphere_density",[("log_density",array()),("altitude_m",scalar())],scalar())
    log,h = b.args
    with b.if_(length(log)<2): b.ret(-1.0)
    xx = b.let("grid_position",math("min",math("max",h/250,0),as_f64(length(log)-1)))
    i = b.let("index",as_i64(xx))
    with b.if_(i>=length(log)-1): b.ret(math("exp",log[length(log)-1]))
    frac = b.let("fraction",xx-as_f64(i))
    b.ret(math("exp",log[i]*(1-frac)+log[i+1]*frac))

    b = fn("thermal_mass_loss",fparams("mass","temperature","rho_m","mu","ca","cb","shape"),scalar())
    mass,temp,rho,mu,ca,cb,shape = b.args
    pv = b.let("vapor_pressure",0.1*math("pow",10.0,ca-cb/temp))
    b.ret(-4*shape*math("pow",mass/rho,2/3)*pv*math("sqrt",mu/(2*PI*1.380649e-23*temp)))

    b = fn("temperature_rate",fparams("mass","velocity","temperature","rho_m","cp","latent","shape","rho_air","mass_loss_positive","heat_transfer","ambient","emissivity"),scalar())
    mass,v,t,rho,cp,latent,shape,air,loss,heat,ambient,emis = b.args
    flux = b.let("flux",0.5*heat*air*v*v*v-4*float(constants.Stefan_Boltzmann)*emis*(t*t*t*t-ambient*ambient*ambient*ambient)
                 -latent/shape*math("pow",rho/mass,2/3)*loss)
    b.ret(shape*flux/(cp*math("pow",mass,1/3)*math("pow",rho,2/3)))

    # Checked SI boundaries around the explicitly normalized numerical kernels.
    # The ODE state/config buffers are mixed-quantity ABI records, not falsely
    # represented as arrays sharing a single physical dimension.
    mass_units=[("mass","kg"),("temperature","K"),("rho_m","kg/m^3"),
                ("mu","kg"),("ca",None),("cb","K"),("shape",None)]
    b = fn("thermal_mass_loss_si",[(name,scalar(unit=unit)) for name,unit in mass_units],scalar(unit="kg/s"))
    raw=[arg/value(1.0,unit=unit) if unit else arg for arg,(_,unit) in zip(b.args,mass_units)]
    b.ret(c("thermal_mass_loss",*raw)*value(1.0,unit="kg/s"))
    heat_units=[("mass","kg"),("velocity","m/s"),("temperature","K"),("rho_m","kg/m^3"),
                ("cp","J/kg/K"),("latent","J/kg"),("shape",None),("rho_air","kg/m^3"),
                ("mass_loss_positive","kg/s"),("heat_transfer",None),("ambient","K"),("emissivity",None)]
    b = fn("temperature_rate_si",[(name,scalar(unit=unit)) for name,unit in heat_units],scalar(unit="K/s"))
    raw=[arg/value(1.0,unit=unit) if unit else arg for arg,(_,unit) in zip(b.args,heat_units)]
    b.ret(c("temperature_rate",*raw)*value(1.0,unit="K/s"))

    b = fn("rhs",[("state",array()),("cfg",array()),("atmosphere",array()),("geometry",array()),("out",array(True))])
    y,cfg,atm,geom,out = b.args
    mass = b.let("mass",math("max",y[0]*cfg[11],cfg[12]*0.001))
    v = b.let("velocity",y[1])
    t = b.let("temperature",math("max",y[3],1))
    h = b.let("height",c("altitude",geom,y[2]))
    air = b.let("air_density",c("atmosphere_density",atm,h))
    dm = b.let("dm",c("thermal_mass_loss_si",mass*value(1.0,unit="kg"),t*value(1.0,unit="K"),
                      cfg[0]*value(1.0,unit="kg/m^3"),cfg[3]*value(1.0,unit="kg"),cfg[4],cfg[5]*value(1.0,unit="K"),cfg[9])/value(1.0,unit="kg/s"))
    rsq = b.let("rsq",(geom[0]-geom[3]*y[2])*(geom[0]-geom[3]*y[2])+(geom[1]-geom[4]*y[2])*(geom[1]-geom[4]*y[2])+(geom[2]-geom[5]*y[2])*(geom[2]-geom[5]*y[2]))
    b.assign(out[0],dm/cfg[11])
    b.assign(out[1],-cfg[7]*cfg[9]*air*v*v/(math("pow",mass,1/3)*math("pow",cfg[0],2/3))+GM/rsq)
    b.assign(out[2],-v)
    thermal_args=[mass,v,t,cfg[0],cfg[1],cfg[2],cfg[9],air,-dm,cfg[6],cfg[10],cfg[8]]
    typed_args=[arg*value(1.0,unit=unit) if unit else arg for arg,(_,unit) in zip(thermal_args,heat_units)]
    b.assign(out[3],c("temperature_rate_si",*typed_args)/value(1.0,unit="K/s"))
    b.ret()

    # Dormand--Prince coefficients are data, copied into the semantic program at
    # authoring. No scipy integrator is called by any Vibe trajectory or search.
    coeff_a = [list(row) for row in RK45.A] + [list(RK45.B)]
    b = fn("rk45_step",[("state",array()),("cfg",array()),("atmosphere",array()),("geometry",array()),("dt",scalar()),("next",array(True)),("stages",array(True))],scalar())
    y,cfg,atm,geom,dt,yn,k = b.args
    stage = b.zeros("stage",4)
    deriv = b.zeros("derivative",4)
    for s,row in enumerate(coeff_a):
        with b.loop(f"component_{s}",4) as i:
            expr = y[i]
            for j,a in enumerate(row):
                if a != 0: expr = expr + dt*float(a)*k[4*j+i]
            b.assign(stage[i],expr)
        invoke(b,"rhs",stage,cfg,atm,geom,deriv)
        with b.loop(f"save_{s}",4) as i: b.assign(k[4*s+i],deriv[i])
    err = b.let("error_sum",0.0,True)
    with b.loop("i",4) as i:
        b.assign(yn[i],stage[i])
        ee = value(0.0)
        for j,a in enumerate(RK45.E):
            if a != 0: ee = ee + float(a)*k[4*j+i]
        tol = b.let("atol",cfg[24+i])
        scaled = b.let("scaled_error",dt*ee/(tol+cfg[14]*math("max",math("abs",y[i]),math("abs",stage[i]))))
        b.assign(err,err+scaled*scaled)
    b.ret(math("sqrt",err/4))

    b = fn("dense_state",[("state",array()),("stages",array()),("dt",scalar()),("theta",scalar()),("out",array(True))])
    y,k,dt,theta,out = b.args
    with b.loop("i",4) as i:
        expr = value(0.0)
        for power in reversed(range(4)):
            term = value(0.0)
            for j,row in enumerate(RK45.P):
                if row[power] != 0: term = term + float(row[power])*k[4*j+i]
            expr = (expr+term)*theta
        b.assign(out[i],y[i]+dt*expr)
    b.ret()

    b = fn("event_value",[("state",array()),("cfg",array()),("geometry",array()),("event",scalar("i64"))],scalar())
    y,cfg,geom,event = b.args
    with b.if_(event==1): b.ret(y[3]-400)
    with b.if_(event==2): b.ret(y[0]-cfg[12]/cfg[11])
    b.ret(c("altitude",geom,y[2]))

    b = fn("integrate",[("cfg",array()),("atmosphere",array()),("speed",scalar()),("zenith",scalar()),("trajectory",array(True)),("summary",array(True))],scalar("i64"))
    cfg,atm,speed,zen,track,summary = b.args
    # Host checks schema lengths; generated indexing remains bounds-checked too.
    y = b.zeros("state",4)
    yn = b.zeros("next",4)
    k = b.zeros("stages",28)
    evstate = b.zeros("event_state",4)
    geom = b.zeros("geometry",6)
    invoke(b,"geometry",cfg,zen,geom)
    b.assign(y[0],1.0); b.assign(y[1],speed); b.assign(y[2],0.0); b.assign(y[3],cfg[10])
    t = b.let("time",0.0,True)
    dt = b.let("dt",math("min",0.001,cfg[13]),True)
    rows = b.let("rows",value(0,"i64"),True)
    rejected = b.let("rejected",value(0,"i64"),True)
    attempts = b.let("attempts",value(0,"i64"),True)
    event_code = b.let("event_code",value(0,"i64"),True)
    tmax = b.let("maximum_temperature",y[3],True)
    peak_h = b.let("peak_height",cfg[18],True)
    height = b.let("height",cfg[18],True)

    def save_row():
        with b.if_(length(track)>=6):
            with b.if_(6*rows+5>=length(track)): b.ret(value(-3,"i64"))
            for j,expr in enumerate([t,y[0]*cfg[11],y[1],y[2],y[3],height]): b.assign(track[6*rows+j],expr)
        b.assign(rows,rows+1)

    save_row()
    with b.while_(t<cfg[20]):
        b.assign(attempts,attempts+1)
        with b.if_(attempts>1000000): b.ret(value(-4,"i64"))
        b.assign(dt,math("min",dt,cfg[20]-t))
        with b.if_(dt<1e-12): b.ret(value(-2,"i64"))
        error = b.let("error",c("rk45_step",y,cfg,atm,geom,dt,yn,k))
        accept = b.let("accept",value(0,"i64"),True)
        with b.if_(math("isfinite",error)):
            with b.if_(error<=1): b.assign(accept,1)
        with b.if_(accept==0):
            b.assign(dt,dt*0.2)
            b.assign(rejected,rejected+1)
        with b.if_(accept==1):
            earliest = b.let("earliest",1.0,True)
            with b.loop("event",4,start=1) as event:
                before = b.let("before",c("event_value",y,cfg,geom,event))
                after = b.let("after",c("event_value",yn,cfg,geom,event))
                with b.if_(before>0):
                    with b.if_(after<=0):
                        lo = b.let("lo",0.0,True); hi = b.let("hi",1.0,True)
                        with b.loop("bisect",40):
                            mid = b.let("mid",0.5*(lo+hi))
                            invoke(b,"dense_state",y,k,dt,mid,evstate)
                            ev = b.let("ev",c("event_value",evstate,cfg,geom,event))
                            with b.if_(ev>0): b.assign(lo,mid)
                            with b.if_(ev<=0): b.assign(hi,mid)
                        fraction = b.let("fraction",0.5*(lo+hi))
                        with b.if_(fraction<earliest):
                            b.assign(earliest,fraction); b.assign(event_code,event)
            with b.if_(earliest<1): invoke(b,"dense_state",y,k,dt,earliest,yn)
            b.assign(t,t+dt*earliest)
            with b.loop("copy",4) as i: b.assign(y[i],yn[i])
            b.assign(height,c("altitude",geom,y[2]))
            with b.if_(y[3]>tmax): b.assign(tmax,y[3]); b.assign(peak_h,height)
            save_row()
            for j,expr in enumerate([tmax,y[0],peak_h,t,height,as_f64(event_code),as_f64(rows-1),as_f64(rejected),speed,cfg[11]]): b.assign(summary[j],expr)
            with b.if_(event_code>0): b.ret(rows)
            factor = b.let("factor",math("min",5,math("max",0.2,0.9*math("pow",math("max",error,1e-16),-0.2))))
            b.assign(dt,math("min",cfg[13],dt*factor))
    b.ret(rows)

    b = fn("objective",[("cfg",array(True)),("atmosphere",array()),("zenith",scalar()),("x",scalar()),("fixed_speed",scalar()),("parameter_index",scalar("i64")),("mode",scalar("i64")),("target",scalar()),("summary",array(True))],scalar())
    cfg,atm,zen,x,fixed,index,mode,target,summary = b.args
    speed = b.let("speed",x,True)
    with b.if_(index>=0):
        b.assign(cfg[index],x)
        b.assign(speed,fixed)
        # Density changes alter initial mass at fixed diameter; cfg[15] is diameter in m.
        with b.if_(index==0):
            b.assign(cfg[11],PI/6*cfg[15]*cfg[15]*cfg[15]*cfg[0])
            b.assign(cfg[12],math("max",cfg[11]*1e-6,1e-18))
    empty = b.zeros("unused_trajectory",1)
    status = b.let("status",c("integrate",cfg,atm,speed,zen,empty,summary,result="i64"))
    with b.if_(status<0):
        b.assign(summary[5],as_f64(status))
        b.ret(0.0)
    with b.if_(mode==1): b.ret(target-summary[1])
    b.ret(summary[0]-target)

    b = fn("boundary",[("cfg",array(True)),("atmosphere",array()),("zenith",scalar()),("target",scalar()),("mode",scalar("i64")),("parameter_index",scalar("i64")),("fixed_speed",scalar()),("lower",scalar()),("upper",scalar()),("xtol",scalar()),("summary",array(True))],scalar())
    cfg,atm,zen,target,mode,index,fixed,lower,upper,xtol,summary = b.args
    low = b.let("low",lower,True); high = b.let("high",upper,True)
    f_low = b.let("f_low",c("objective",cfg,atm,zen,low,fixed,index,mode,target,summary),True)
    with b.if_(summary[5]<0): b.ret(-1.0)
    f_high = b.let("f_high",c("objective",cfg,atm,zen,high,fixed,index,mode,target,summary),True)
    with b.if_(summary[5]<0): b.ret(-1.0)
    with b.if_(index<0):
        with b.while_(f_low>0):
            b.assign(low,low*0.75)
            with b.if_(low<1): b.ret(-2.0)
            b.assign(f_low,c("objective",cfg,atm,zen,low,fixed,index,mode,target,summary))
            with b.if_(summary[5]<0): b.ret(-1.0)
        with b.while_(f_high<0):
            b.assign(high,high*1.25)
            with b.if_(high>80000): b.ret(-2.0)
            b.assign(f_high,c("objective",cfg,atm,zen,high,fixed,index,mode,target,summary))
            with b.if_(summary[5]<0): b.ret(-1.0)
    with b.if_(f_low*f_high>0): b.ret(-2.0)
    with b.while_(high-low>xtol):
        mid = b.let("mid",0.5*(low+high))
        fm = b.let("f_mid",c("objective",cfg,atm,zen,mid,fixed,index,mode,target,summary))
        with b.if_(summary[5]<0): b.ret(-1.0)
        bracket_left = b.let("bracket_left",fm*f_low<=0)
        same_side = b.let("same_side",fm*f_low>0)
        with b.if_(bracket_left): b.assign(high,mid)
        with b.if_(same_side): b.assign(low,mid); b.assign(f_low,fm)
    root = b.let("root",0.5*(low+high))
    b.invoke(PREFIX+"objective",cfg,atm,zen,root,fixed,index,mode,target,summary)
    with b.if_(summary[5]<0): b.ret(-1.0)
    b.ret(root)

    b = fn("tangent_discriminant",[("cfg",array()),("zenith",scalar())],scalar())
    cfg,zen = b.args
    geom = b.zeros("geometry",6); invoke(b,"geometry",cfg,zen,geom)
    ab = A_EARTH*(1-1/298.257223563)
    pp = b.let("pp",geom[0]*geom[0]/(A_EARTH*A_EARTH)+geom[1]*geom[1]/(A_EARTH*A_EARTH)+geom[2]*geom[2]/(ab*ab))
    pu = b.let("pu",geom[0]*geom[3]/(A_EARTH*A_EARTH)+geom[1]*geom[4]/(A_EARTH*A_EARTH)+geom[2]*geom[5]/(ab*ab))
    uu = b.let("uu",geom[3]*geom[3]/(A_EARTH*A_EARTH)+geom[4]*geom[4]/(A_EARTH*A_EARTH)+geom[5]*geom[5]/(ab*ab))
    b.ret(pu*pu-uu*(pp-1))

    b = fn("tangent_zenith",[("cfg",array())],scalar())
    cfg, = b.args
    low = b.let("low",60.0,True); high = b.let("high",89.0,True)
    with b.loop("i",50):
        mid = b.let("mid",0.5*(low+high))
        disc = b.let("discriminant",c("tangent_discriminant",cfg,mid))
        with b.if_(disc>0): b.assign(low,mid)
        with b.if_(disc<=0): b.assign(high,mid)
    b.ret(0.5*(low+high))

    b = fn("escape_speed",[("cfg",array())],scalar())
    cfg, = b.args
    xyz = b.zeros("xyz",3); invoke(b,"geodetic_to_ecef",cfg[16],cfg[17],cfg[18],xyz)
    b.ret(math("sqrt",2*3.986004418e14/math("sqrt",xyz[0]*xyz[0]+xyz[1]*xyz[1]+xyz[2]*xyz[2])))

    b = fn("radiation_pressure",fparams("radius","density","qpr","distance")+[("out",array(True))])
    radius,rho,qpr,distance,out = b.args
    coeff = b.let("coeff",3*3.828e26*qpr/(16*PI*299792458.0*6.67430e-11*1.98847e30*rho))
    b.assign(out[0],coeff/radius)
    b.assign(out[1],4*PI*rho*radius*299792458.0*299792458.0*distance*distance/(3*3.828e26*qpr))
    b.assign(out[2],coeff/0.5); b.assign(out[3],coeff)
    b.ret()
    return p


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pack",type=Path,default=ROOT/"examples/metablate/metablate.vibepack")
    args=parser.parse_args()
    print(make_program().apply(ROOT/"target/debug/vibec",args.pack))


if __name__ == "__main__": main()
