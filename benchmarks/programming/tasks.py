"""Small paired scientific programming tasks and controller-only calibration solutions."""
import math
import textwrap
import numpy as np

TASKS = {
    "stable_norm": ("implement", "Return the Euclidean norm of all input values. Empty input returns 0. Inputs are finite, including magnitudes from 1e-300 to 1e300. Avoid spurious overflow/underflow."),
    "trapezoid": ("debug", "Fix the supplied integrator. Input is flattened (x,y) pairs with strictly increasing x and at least two points. Return the nonuniform-grid composite trapezoidal integral, including every interval."),
    "weighted_mean": ("modify", "Extend the supplied arithmetic mean to a weighted mean. Input is flattened (value,weight) pairs with nonnegative finite weights. Ignore zero weights; empty input or zero total weight returns 0. Preserve the unit-weight arithmetic-mean behavior. Magnitudes are bounded by 1e4."),
    "logsumexp": ("implement", "Return log(sum(exp(x))) stably for finite inputs in [-1000,1000]. Empty input returns negative infinity. Singleton input returns that input."),
}


def cases(task, seed, count):
    rng=np.random.default_rng(seed)
    if task=="stable_norm":
        inputs=[[],[3,4],[1e300,1e300],[1e-300,-1e-300],[0,0]]
        inputs += [(rng.normal(size=int(rng.integers(1,40)))*10.0**rng.uniform(-300,300)).tolist() for _ in range(count)]
        expected=[math.hypot(*x) for x in inputs]
    elif task=="trapezoid":
        inputs=[[0,0,1,2],[0,1,1,1,4,1],[-2,3,0,-1,2,4]]
        for _ in range(count):
            n=int(rng.integers(2,30)); x=np.cumsum(rng.uniform(.01,3,n)); y=rng.normal(size=n)*10
            inputs.append(np.column_stack([x,y]).ravel().tolist())
        expected=[math.fsum((v[i+2]-v[i])*(v[i+1]+v[i+3])/2 for i in range(0,len(v)-2,2)) for v in inputs]
    elif task=="weighted_mean":
        inputs=[[],[1,0,99,0],[2,1,8,3],[2,1,8,1],[100,0,-3,2]]
        for _ in range(count):
            n=int(rng.integers(1,30)); values=rng.uniform(-1e4,1e4,n); weights=rng.uniform(0,10,n)
            weights[rng.random(n)<.25]=0
            inputs.append(np.column_stack([values,weights]).ravel().tolist())
        expected=[math.fsum(v[i]*v[i+1] for i in range(0,len(v),2))/math.fsum(v[1::2]) if math.fsum(v[1::2]) else 0 for v in inputs]
    else:
        inputs=[[],[1000,1000],[-1000,-1000],[0],[1000,-1000]]
        inputs += [rng.uniform(-1000,1000,int(rng.integers(1,40))).tolist() for _ in range(count)]
        expected=[max(x)+math.log(math.fsum(math.exp(v-max(x)) for v in x)) if x else -math.inf for x in inputs]
    return inputs,np.asarray(expected,dtype=np.float64)


def python_source(task, solution=False):
    bodies={
        "stable_norm":"return math.hypot(*data)",
        "trapezoid":"return sum((data[i+2]-data[i])*(data[i+1]+data[i+3])/2 for i in range(0,len(data)-2,2))",
        "weighted_mean":"weight=sum(data[1::2])\nreturn sum(data[i]*data[i+1] for i in range(0,len(data),2))/weight if weight else 0.0",
        "logsumexp":"if not data: return -math.inf\nm=max(data)\nreturn m+math.log(sum(math.exp(x-m) for x in data))",
    }
    if not solution:
        bodies["trapezoid"]=bodies["trapezoid"].replace("len(data)-2","len(data)-4")
        bodies["weighted_mean"]="return sum(data[::2])/(len(data)//2) if data else 0.0"
        bodies["stable_norm"]=bodies["logsumexp"]="return 0.0  # TODO"
    return "import math\n\ndef solve(data):\n"+textwrap.indent(bodies[task],"    ")+"\n"


def rust_source(task, solution=False):
    bodies={
        "stable_norm":"data.iter().fold(0.0_f64, |a, &x| a.hypot(x))",
        "trapezoid":"let mut total=0.0; for i in (0..data.len().saturating_sub(2)).step_by(2) { total+=(data[i+2]-data[i])*(data[i+1]+data[i+3])/2.0; } total",
        "weighted_mean":"let mut weight=0.0; let mut total=0.0; for p in data.chunks_exact(2) {total+=p[0]*p[1]; weight+=p[1];} if weight==0.0 {0.0} else {total/weight}",
        "logsumexp":"if data.is_empty() {return f64::NEG_INFINITY;} let m=data.iter().copied().fold(f64::NEG_INFINITY,f64::max); m+data.iter().map(|x|(x-m).exp()).sum::<f64>().ln()",
    }
    if not solution:
        bodies["trapezoid"]=bodies["trapezoid"].replace("saturating_sub(2)","saturating_sub(4)")
        bodies["weighted_mean"]="if data.is_empty() {0.0} else {data.iter().step_by(2).sum::<f64>() / (data.len()/2) as f64}"
        bodies["stable_norm"]=bodies["logsumexp"]="0.0 // TODO\n"
    return "pub fn solve(data: &[f64]) -> f64 {\n"+textwrap.indent(bodies[task],"    ")+"\n}\n"


def vibe_body(task, solution=False):
    bodies={
        "stable_norm": '''
scale=b.let("scale",0.0,True)
with b.loop("i",length(data)) as i:
    b.assign(scale,math("max",scale,math("abs",data[i])))
with b.if_(scale==0.0): b.ret(0.0)
total=b.let("total",0.0,True)
with b.loop("j",length(data)) as j:
    q=b.let("q",data[j]/scale)
    b.assign(total,total+q*q)
b.ret(scale*math("sqrt",total))
''',
        "trapezoid": '''
total=b.let("total",0.0,True)
with b.loop("i",length(data)/2-1) as i:
    j=b.let("j",i*2)
    b.assign(total,total+(data[j+2]-data[j])*(data[j+1]+data[j+3])/2)
b.ret(total)
''',
        "weighted_mean": '''
total=b.let("total",0.0,True)
weight=b.let("weight",0.0,True)
with b.loop("i",length(data)/2) as i:
    b.assign(total,total+data[i*2]*data[i*2+1])
    b.assign(weight,weight+data[i*2+1])
with b.if_(weight==0.0): b.ret(0.0)
b.ret(total/weight)
''',
        "logsumexp": '''
with b.if_(length(data)==0): b.ret(math("log",0.0))
maximum=b.let("maximum",data[0],True)
with b.loop("i",length(data)) as i:
    b.assign(maximum,math("max",maximum,data[i]))
total=b.let("total",0.0,True)
with b.loop("j",length(data)) as j:
    b.assign(total,total+math("exp",data[j]-maximum))
b.ret(maximum+math("log",total))
'''}
    if not solution:
        bodies["trapezoid"]=bodies["trapezoid"].replace("length(data)/2-1","length(data)/2-2")
        bodies["weighted_mean"]='''
with b.if_(length(data)==0): b.ret(0.0)
total=b.let("total",0.0,True)
with b.loop("i",length(data)/2) as i: b.assign(total,total+data[i*2])
b.ret(total/as_f64(length(data)/2))
'''
        bodies["stable_norm"]=bodies["logsumexp"]="b.ret(0.0) # TODO\n"
    return textwrap.dedent(bodies[task]).strip()+"\n"
