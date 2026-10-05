# metablate — readable program

Generated from [metablate.vibepack](metablate.vibepack). **Do not edit this companion by hand.**

This is a compiler-derived pseudocode view of the stored program, not an executable source file or a correctness certificate. Numeric types, declared units, mutation and branch structure are retained. Unmarked integers use i64; decimals and short math calls use f64. Other literal types are shown. `range(start, end)` excludes the end. Long strings may be explicitly omitted by the compiler query. Internal revisions and hashes are intentionally not displayed.

Build the compiler with `cargo build`, then regenerate from the repository root: `conda run -n base python scripts/render_packs.py`.

## Functions

| Function | Calls |
|---|---|
| [@metablate.altitude](#function-1) | [@metablate.ecef_to_geodetic](#function-5) |
| [@metablate.atmosphere_density](#function-2) | None |
| [@metablate.boundary](#function-3) | [@metablate.objective](#function-11) |
| [@metablate.dense_state](#function-4) | None |
| [@metablate.ecef_to_geodetic](#function-5) | None |
| [@metablate.escape_speed](#function-6) | [@metablate.geodetic_to_ecef](#function-8) |
| [@metablate.event_value](#function-7) | [@metablate.altitude](#function-1) |
| [@metablate.geodetic_to_ecef](#function-8) | None |
| [@metablate.geometry](#function-9) | [@metablate.geodetic_to_ecef](#function-8) |
| [@metablate.integrate](#function-10) | [@metablate.altitude](#function-1), [@metablate.dense_state](#function-4), [@metablate.event_value](#function-7), [@metablate.geometry](#function-9), [@metablate.rk45_step](#function-15) |
| [@metablate.objective](#function-11) | [@metablate.integrate](#function-10) |
| [@metablate.particle_mass](#function-12) | None |
| [@metablate.radiation_pressure](#function-13) | None |
| [@metablate.rhs](#function-14) | [@metablate.altitude](#function-1), [@metablate.atmosphere_density](#function-2), [@metablate.temperature_rate_si](#function-19), [@metablate.thermal_mass_loss_si](#function-21) |
| [@metablate.rk45_step](#function-15) | [@metablate.rhs](#function-14) |
| [@metablate.tangent_discriminant](#function-16) | [@metablate.geometry](#function-9) |
| [@metablate.tangent_zenith](#function-17) | [@metablate.tangent_discriminant](#function-16) |
| [@metablate.temperature_rate](#function-18) | None |
| [@metablate.temperature_rate_si](#function-19) | [@metablate.temperature_rate](#function-18) |
| [@metablate.thermal_mass_loss](#function-20) | None |
| [@metablate.thermal_mass_loss_si](#function-21) | [@metablate.thermal_mass_loss](#function-20) |

<a id="function-1"></a>
### @metablate.altitude

Called by: [@metablate.event_value](#function-7), [@metablate.integrate](#function-10), [@metablate.rhs](#function-14). Calls: [@metablate.ecef_to_geodetic](#function-5).

<details>
<summary>View implementation</summary>

```text
function @metablate.altitude(geometry: Array<f64,1>, position: f64) -> f64:
    var geo = [0.0, 0.0, 0.0]
    ecef_to_geodetic((geometry[0] - (geometry[3] * position)), (geometry[1] - (geometry[4] * position)), (geometry[2] - (geometry[5] * position)), geo)
    return geo[2]
```

</details>

<a id="function-2"></a>
### @metablate.atmosphere_density

Called by: [@metablate.rhs](#function-14). Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.atmosphere_density(log_density: Array<f64,1>, altitude_m: f64) -> f64:
    if (len(log_density) < 2):
        return -1.0
    let grid_position = min(max((altitude_m / 250.0), 0.0), @cast.f64_from_i64((len(log_density) - 1)))
    let index = @cast.i64_from_f64(grid_position)
    if (index >= (len(log_density) - 1)):
        return exp(log_density[(len(log_density) - 1)])
    let fraction = (grid_position - @cast.f64_from_i64(index))
    return exp(((log_density[index] * (1.0 - fraction)) + (log_density[(index + 1)] * fraction)))
```

</details>

<a id="function-3"></a>
### @metablate.boundary

Called by: None. Calls: [@metablate.objective](#function-11).

<details>
<summary>View implementation</summary>

```text
function @metablate.boundary(cfg: mut Array<f64,1>, atmosphere: Array<f64,1>, zenith: f64, target: f64, mode: i64, parameter_index: i64, fixed_speed: f64, lower: f64, upper: f64, xtol: f64, summary: mut Array<f64,1>) -> f64:
    var low = lower
    var high = upper
    var f_low = objective(cfg, atmosphere, zenith, low, fixed_speed, parameter_index, mode, target, summary)
    if (summary[5] < 0.0):
        return -1.0
    var f_high = objective(cfg, atmosphere, zenith, high, fixed_speed, parameter_index, mode, target, summary)
    if (summary[5] < 0.0):
        return -1.0
    if (parameter_index < 0):
        while (f_low > 0.0):
            low = (low * 0.75)
            if (low < 1.0):
                return -2.0
            f_low = objective(cfg, atmosphere, zenith, low, fixed_speed, parameter_index, mode, target, summary)
            if (summary[5] < 0.0):
                return -1.0
        while (f_high < 0.0):
            high = (high * 1.25)
            if (high > 80000.0):
                return -2.0
            f_high = objective(cfg, atmosphere, zenith, high, fixed_speed, parameter_index, mode, target, summary)
            if (summary[5] < 0.0):
                return -1.0
    if ((f_low * f_high) > 0.0):
        return -2.0
    while ((high - low) > xtol):
        let mid = (0.5 * (low + high))
        let f_mid = objective(cfg, atmosphere, zenith, mid, fixed_speed, parameter_index, mode, target, summary)
        if (summary[5] < 0.0):
            return -1.0
        let bracket_left = ((f_mid * f_low) <= 0.0)
        let same_side = ((f_mid * f_low) > 0.0)
        if bracket_left:
            high = mid
        if same_side:
            low = mid
            f_low = f_mid
    let root = (0.5 * (low + high))
    objective(cfg, atmosphere, zenith, root, fixed_speed, parameter_index, mode, target, summary)
    if (summary[5] < 0.0):
        return -1.0
    return root
```

</details>

<a id="function-4"></a>
### @metablate.dense_state

Called by: [@metablate.integrate](#function-10). Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.dense_state(state: Array<f64,1>, stages: Array<f64,1>, dt: f64, theta: f64, out: mut Array<f64,1>) -> none:
    for i in range(0, 4):
        out[i] = (state[i] + (dt * ((((((((0.0 + ((((((0.0 + (-1.1270175653862835 * stages[(0 + i)])) + (2.675424484351598 * stages[(8 + i)])) + (-5.685526961588504 * stages[(12 + i)])) + (3.5219323679207912 * stages[(16 + i)])) + (-1.7672812570757455 * stages[(20 + i)])) + (2.382468931778144 * stages[(24 + i)]))) * theta) + ((((((0.0 + (3.0717434641059005 * stages[(0 + i)])) + (-6.249321565289 * stages[(8 + i)])) + (10.068970589843675 * stages[(12 + i)])) + (-6.399112377351017 * stages[(16 + i)])) + (3.272657752246729 * stages[(20 + i)])) + (-3.764937863556287 * stages[(24 + i)]))) * theta) + ((((((0.0 + (-2.8535800653862835 * stages[(0 + i)])) + (4.023133379230305 * stages[(8 + i)])) + (-3.7324019615885042 * stages[(12 + i)])) + (2.5548038301849423 * stages[(16 + i)])) + (-1.3744241142186024 * stages[(20 + i)])) + (1.3824689317781436 * stages[(24 + i)]))) * theta) + (0.0 + (1.0 * stages[(0 + i)]))) * theta)))
    return
```

</details>

<a id="function-5"></a>
### @metablate.ecef_to_geodetic

Called by: [@metablate.altitude](#function-1). Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.ecef_to_geodetic(x: f64, y: f64, z: f64, out: mut Array<f64,1>) -> none:
    let radius_xy = sqrt(((x * x) + (y * y)))
    if (radius_xy < 1e-09):
        out[0] = 90.0
        if (z < 0.0):
            out[0] = -90.0
        if (z == 0.0):
            out[0] = 0.0
        out[1] = 0.0
        out[2] = (abs(z) - 6356752.3142)
        return
    let ff = ((2182048199140700.8 * z) * z)
    let gg = (((radius_xy * radius_xy) + ((0.99330562000986 * z) * z)) - 1823091258.4542913)
    let cc = ((((4.4814723452386825e-05 * ff) * radius_xy) * radius_xy) / ((gg * gg) * gg))
    let ss = cbrt(((1.0 + cc) + sqrt(((cc * cc) + (2.0 * cc)))))
    let pp = (ff / ((((3.0 * ((ss + (1.0 / ss)) + 1.0)) * ((ss + (1.0 / ss)) + 1.0)) * gg) * gg))
    let qq = sqrt((1.0 + (8.962944690477365e-05 * pp)))
    let r0 = (((((-pp) * 0.00669437999014) * radius_xy) / (1.0 + qq)) + sqrt((((20340315795384.5 * (1.0 + (1.0 / qq))) - ((((pp * 0.99330562000986) * z) * z) / (qq * (1.0 + qq)))) - (((0.5 * pp) * radius_xy) * radius_xy))))
    let uu = sqrt((((radius_xy - (0.00669437999014 * r0)) * (radius_xy - (0.00669437999014 * r0))) + (z * z)))
    let vv = sqrt((((radius_xy - (0.00669437999014 * r0)) * (radius_xy - (0.00669437999014 * r0))) + ((0.99330562000986 * z) * z)))
    let z0 = ((40408299984087.055 * z) / (6378137.0 * vv))
    out[0] = ((atan(((z + (0.00673949674228 * z0)) / radius_xy)) * 180.0) / 3.141592653589793)
    out[1] = ((atan2(y, x) * 180.0) / 3.141592653589793)
    out[2] = (uu * (1.0 - (40408299984087.055 / (6378137.0 * vv))))
    return
```

</details>

<a id="function-6"></a>
### @metablate.escape_speed

Called by: None. Calls: [@metablate.geodetic_to_ecef](#function-8).

<details>
<summary>View implementation</summary>

```text
function @metablate.escape_speed(cfg: Array<f64,1>) -> f64:
    var xyz = [0.0, 0.0, 0.0]
    geodetic_to_ecef(cfg[16], cfg[17], cfg[18], xyz)
    return sqrt((797200883600000.0 / sqrt((((xyz[0] * xyz[0]) + (xyz[1] * xyz[1])) + (xyz[2] * xyz[2])))))
```

</details>

<a id="function-7"></a>
### @metablate.event_value

Called by: [@metablate.integrate](#function-10). Calls: [@metablate.altitude](#function-1).

<details>
<summary>View implementation</summary>

```text
function @metablate.event_value(state: Array<f64,1>, cfg: Array<f64,1>, geometry: Array<f64,1>, event: i64) -> f64:
    if (event == 1):
        return (state[3] - 400.0)
    if (event == 2):
        return (state[0] - (cfg[12] / cfg[11]))
    return altitude(geometry, state[2])
```

</details>

<a id="function-8"></a>
### @metablate.geodetic_to_ecef

Called by: [@metablate.escape_speed](#function-6), [@metablate.geometry](#function-9). Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.geodetic_to_ecef(latitude_deg: f64, longitude_deg: f64, altitude_m: f64, out: mut Array<f64,1>) -> none:
    let latitude = ((latitude_deg * 3.141592653589793) / 180.0)
    let longitude = ((longitude_deg * 3.141592653589793) / 180.0)
    let xi = sqrt((1.0 - ((0.00669437999014 * sin(latitude)) * sin(latitude))))
    out[0] = ((((6378137.0 / xi) + altitude_m) * cos(latitude)) * cos(longitude))
    out[1] = ((((6378137.0 / xi) + altitude_m) * cos(latitude)) * sin(longitude))
    out[2] = ((((6378137.0 / xi) * 0.99330562000986) + altitude_m) * sin(latitude))
    return
```

</details>

<a id="function-9"></a>
### @metablate.geometry

Called by: [@metablate.integrate](#function-10), [@metablate.tangent_discriminant](#function-16). Calls: [@metablate.geodetic_to_ecef](#function-8).

<details>
<summary>View implementation</summary>

```text
function @metablate.geometry(cfg: Array<f64,1>, zenith_deg: f64, out: mut Array<f64,1>) -> none:
    var xyz = [0.0, 0.0, 0.0]
    geodetic_to_ecef(cfg[16], cfg[17], cfg[18], xyz)
    let lat = ((cfg[16] * 3.141592653589793) / 180.0)
    let lon = ((cfg[17] * 3.141592653589793) / 180.0)
    let zenith = ((zenith_deg * 3.141592653589793) / 180.0)
    let north = (-sin(zenith))
    let up = (-cos(zenith))
    for i in range(0, 3):
        out[i] = xyz[i]
    out[3] = ((((-sin(lat)) * cos(lon)) * north) + ((cos(lat) * cos(lon)) * up))
    out[4] = ((((-sin(lat)) * sin(lon)) * north) + ((cos(lat) * sin(lon)) * up))
    out[5] = ((cos(lat) * north) + (sin(lat) * up))
    return
```

</details>

<a id="function-10"></a>
### @metablate.integrate

Called by: [@metablate.objective](#function-11). Calls: [@metablate.altitude](#function-1), [@metablate.dense_state](#function-4), [@metablate.event_value](#function-7), [@metablate.geometry](#function-9), [@metablate.rk45_step](#function-15).

<details>
<summary>View implementation</summary>

```text
function @metablate.integrate(cfg: Array<f64,1>, atmosphere: Array<f64,1>, speed: f64, zenith: f64, trajectory: mut Array<f64,1>, summary: mut Array<f64,1>) -> i64:
    var state = [0.0, 0.0, 0.0, 0.0]
    var next = [0.0, 0.0, 0.0, 0.0]
    var stages = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    var event_state = [0.0, 0.0, 0.0, 0.0]
    var geometry = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    geometry(cfg, zenith, geometry)
    state[0] = 1.0
    state[1] = speed
    state[2] = 0.0
    state[3] = cfg[10]
    var time = 0.0
    var dt = min(0.001, cfg[13])
    var rows = 0
    var rejected = 0
    var attempts = 0
    var event_code = 0
    var maximum_temperature = state[3]
    var peak_height = cfg[18]
    var height = cfg[18]
    if (len(trajectory) >= 6):
        if (((6 * rows) + 5) >= len(trajectory)):
            return -3
        trajectory[((6 * rows) + 0)] = time
        trajectory[((6 * rows) + 1)] = (state[0] * cfg[11])
        trajectory[((6 * rows) + 2)] = state[1]
        trajectory[((6 * rows) + 3)] = state[2]
        trajectory[((6 * rows) + 4)] = state[3]
        trajectory[((6 * rows) + 5)] = height
    rows = (rows + 1)
    while (time < cfg[20]):
        attempts = (attempts + 1)
        if (attempts > 1000000):
            return -4
        dt = min(dt, (cfg[20] - time))
        if (dt < 1e-12):
            return -2
        let error = rk45_step(state, cfg, atmosphere, geometry, dt, next, stages)
        var accept = 0
        if isfinite(error):
            if (error <= 1.0):
                accept = 1
        if (accept == 0):
            dt = (dt * 0.2)
            rejected = (rejected + 1)
        if (accept == 1):
            var earliest = 1.0
            for event in range(1, 4):
                let before = event_value(state, cfg, geometry, event)
                let after = event_value(next, cfg, geometry, event)
                if (before > 0.0):
                    if (after <= 0.0):
                        var lo = 0.0
                        var hi = 1.0
                        for bisect in range(0, 40):
                            let mid = (0.5 * (lo + hi))
                            dense_state(state, stages, dt, mid, event_state)
                            let ev = event_value(event_state, cfg, geometry, event)
                            if (ev > 0.0):
                                lo = mid
                            if (ev <= 0.0):
                                hi = mid
                        let fraction = (0.5 * (lo + hi))
                        if (fraction < earliest):
                            earliest = fraction
                            event_code = event
            if (earliest < 1.0):
                dense_state(state, stages, dt, earliest, next)
            time = (time + (dt * earliest))
            for copy in range(0, 4):
                state[copy] = next[copy]
            height = altitude(geometry, state[2])
            if (state[3] > maximum_temperature):
                maximum_temperature = state[3]
                peak_height = height
            if (len(trajectory) >= 6):
                if (((6 * rows) + 5) >= len(trajectory)):
                    return -3
                trajectory[((6 * rows) + 0)] = time
                trajectory[((6 * rows) + 1)] = (state[0] * cfg[11])
                trajectory[((6 * rows) + 2)] = state[1]
                trajectory[((6 * rows) + 3)] = state[2]
                trajectory[((6 * rows) + 4)] = state[3]
                trajectory[((6 * rows) + 5)] = height
            rows = (rows + 1)
            summary[0] = maximum_temperature
            summary[1] = state[0]
            summary[2] = peak_height
            summary[3] = time
            summary[4] = height
            summary[5] = @cast.f64_from_i64(event_code)
            summary[6] = @cast.f64_from_i64((rows - 1))
            summary[7] = @cast.f64_from_i64(rejected)
            summary[8] = speed
            summary[9] = cfg[11]
            if (event_code > 0):
                return rows
            let factor = min(5.0, max(0.2, (0.9 * pow(max(error, 1e-16), -0.2))))
            dt = min(cfg[13], (dt * factor))
    return rows
```

</details>

<a id="function-11"></a>
### @metablate.objective

Called by: [@metablate.boundary](#function-3). Calls: [@metablate.integrate](#function-10).

<details>
<summary>View implementation</summary>

```text
function @metablate.objective(cfg: mut Array<f64,1>, atmosphere: Array<f64,1>, zenith: f64, x: f64, fixed_speed: f64, parameter_index: i64, mode: i64, target: f64, summary: mut Array<f64,1>) -> f64:
    var speed = x
    if (parameter_index >= 0):
        cfg[parameter_index] = x
        speed = fixed_speed
        if (parameter_index == 0):
            cfg[11] = ((((0.5235987755982988 * cfg[15]) * cfg[15]) * cfg[15]) * cfg[0])
            cfg[12] = max((cfg[11] * 1e-06), 1e-18)
    var unused_trajectory = [0.0]
    let status = integrate(cfg, atmosphere, speed, zenith, unused_trajectory, summary)
    if (status < 0):
        summary[5] = @cast.f64_from_i64(status)
        return 0.0
    if (mode == 1):
        return (target - summary[1])
    return (summary[0] - target)
```

</details>

<a id="function-12"></a>
### @metablate.particle_mass

Called by: None. Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.particle_mass(diameter: f64[m], density: f64[kg/m^3]) -> f64[kg]:
    return ((((0.5235987755982988 * diameter) * diameter) * diameter) * density)
```

</details>

<a id="function-13"></a>
### @metablate.radiation_pressure

Called by: None. Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.radiation_pressure(radius: f64, density: f64, qpr: f64, distance: f64, out: mut Array<f64,1>) -> none:
    let coeff = ((1.1484e+27 * qpr) / (1.999932440773508e+30 * density))
    out[0] = (coeff / radius)
    out[1] = (((((((12.566370614359172 * density) * radius) * 299792458.0) * 299792458.0) * distance) * distance) / (1.1484e+27 * qpr))
    out[2] = (coeff / 0.5)
    out[3] = coeff
    return
```

</details>

<a id="function-14"></a>
### @metablate.rhs

Called by: [@metablate.rk45_step](#function-15). Calls: [@metablate.altitude](#function-1), [@metablate.atmosphere_density](#function-2), [@metablate.temperature_rate_si](#function-19), [@metablate.thermal_mass_loss_si](#function-21).

<details>
<summary>View implementation</summary>

```text
function @metablate.rhs(state: Array<f64,1>, cfg: Array<f64,1>, atmosphere: Array<f64,1>, geometry: Array<f64,1>, out: mut Array<f64,1>) -> none:
    let mass = max((state[0] * cfg[11]), (cfg[12] * 0.001))
    let velocity = state[1]
    let temperature = max(state[3], 1.0)
    let height = altitude(geometry, state[2])
    let air_density = atmosphere_density(atmosphere, height)
    let dm = (thermal_mass_loss_si((mass * 1.0 [kg]), (temperature * 1.0 [K]), (cfg[0] * 1.0 [kg/m^3]), (cfg[3] * 1.0 [kg]), cfg[4], (cfg[5] * 1.0 [K]), cfg[9]) / 1.0 [kg/s])
    let rsq = ((((geometry[0] - (geometry[3] * state[2])) * (geometry[0] - (geometry[3] * state[2]))) + ((geometry[1] - (geometry[4] * state[2])) * (geometry[1] - (geometry[4] * state[2])))) + ((geometry[2] - (geometry[5] * state[2])) * (geometry[2] - (geometry[5] * state[2]))))
    out[0] = (dm / cfg[11])
    out[1] = (((((((-cfg[7]) * cfg[9]) * air_density) * velocity) * velocity) / (pow(mass, 0.3333333333333333) * pow(cfg[0], 0.6666666666666666))) + (398736030600000.0 / rsq))
    out[2] = (-velocity)
    out[3] = (temperature_rate_si((mass * 1.0 [kg]), (velocity * 1.0 [m/s]), (temperature * 1.0 [K]), (cfg[0] * 1.0 [kg/m^3]), (cfg[1] * 1.0 [J/kg/K]), (cfg[2] * 1.0 [J/kg]), cfg[9], (air_density * 1.0 [kg/m^3]), ((-dm) * 1.0 [kg/s]), cfg[6], (cfg[10] * 1.0 [K]), cfg[8]) / 1.0 [K/s])
    return
```

</details>

<a id="function-15"></a>
### @metablate.rk45_step

Called by: [@metablate.integrate](#function-10). Calls: [@metablate.rhs](#function-14).

<details>
<summary>View implementation</summary>

```text
function @metablate.rk45_step(state: Array<f64,1>, cfg: Array<f64,1>, atmosphere: Array<f64,1>, geometry: Array<f64,1>, dt: f64, next: mut Array<f64,1>, stages: mut Array<f64,1>) -> f64:
    var stage = [0.0, 0.0, 0.0, 0.0]
    var derivative = [0.0, 0.0, 0.0, 0.0]
    for component_0 in range(0, 4):
        stage[component_0] = state[component_0]
    rhs(stage, cfg, atmosphere, geometry, derivative)
    for save_0 in range(0, 4):
        stages[(0 + save_0)] = derivative[save_0]
    for component_1 in range(0, 4):
        stage[component_1] = (state[component_1] + ((dt * 0.2) * stages[(0 + component_1)]))
    rhs(stage, cfg, atmosphere, geometry, derivative)
    for save_1 in range(0, 4):
        stages[(4 + save_1)] = derivative[save_1]
    for component_2 in range(0, 4):
        stage[component_2] = ((state[component_2] + ((dt * 0.075) * stages[(0 + component_2)])) + ((dt * 0.225) * stages[(4 + component_2)]))
    rhs(stage, cfg, atmosphere, geometry, derivative)
    for save_2 in range(0, 4):
        stages[(8 + save_2)] = derivative[save_2]
    for component_3 in range(0, 4):
        stage[component_3] = (((state[component_3] + ((dt * 0.9777777777777777) * stages[(0 + component_3)])) + ((dt * -3.7333333333333334) * stages[(4 + component_3)])) + ((dt * 3.5555555555555554) * stages[(8 + component_3)]))
    rhs(stage, cfg, atmosphere, geometry, derivative)
    for save_3 in range(0, 4):
        stages[(12 + save_3)] = derivative[save_3]
    for component_4 in range(0, 4):
        stage[component_4] = ((((state[component_4] + ((dt * 2.9525986892242035) * stages[(0 + component_4)])) + ((dt * -11.595793324188385) * stages[(4 + component_4)])) + ((dt * 9.822892851699436) * stages[(8 + component_4)])) + ((dt * -0.2908093278463649) * stages[(12 + component_4)]))
    rhs(stage, cfg, atmosphere, geometry, derivative)
    for save_4 in range(0, 4):
        stages[(16 + save_4)] = derivative[save_4]
    for component_5 in range(0, 4):
        stage[component_5] = (((((state[component_5] + ((dt * 2.8462752525252526) * stages[(0 + component_5)])) + ((dt * -10.757575757575758) * stages[(4 + component_5)])) + ((dt * 8.906422717743473) * stages[(8 + component_5)])) + ((dt * 0.2784090909090909) * stages[(12 + component_5)])) + ((dt * -0.2735313036020583) * stages[(16 + component_5)]))
    rhs(stage, cfg, atmosphere, geometry, derivative)
    for save_5 in range(0, 4):
        stages[(20 + save_5)] = derivative[save_5]
    for component_6 in range(0, 4):
        stage[component_6] = (((((state[component_6] + ((dt * 0.09114583333333333) * stages[(0 + component_6)])) + ((dt * 0.44923629829290207) * stages[(8 + component_6)])) + ((dt * 0.6510416666666666) * stages[(12 + component_6)])) + ((dt * -0.322376179245283) * stages[(16 + component_6)])) + ((dt * 0.13095238095238096) * stages[(20 + component_6)]))
    rhs(stage, cfg, atmosphere, geometry, derivative)
    for save_6 in range(0, 4):
        stages[(24 + save_6)] = derivative[save_6]
    var error_sum = 0.0
    for i in range(0, 4):
        next[i] = stage[i]
        let atol = cfg[(24 + i)]
        let scaled_error = ((dt * ((((((0.0 + (-0.0012326388888888888 * stages[(0 + i)])) + (0.0042527702905061394 * stages[(8 + i)])) + (-0.03697916666666667 * stages[(12 + i)])) + (0.05086379716981132 * stages[(16 + i)])) + (-0.0419047619047619 * stages[(20 + i)])) + (0.025 * stages[(24 + i)]))) / (atol + (cfg[14] * max(abs(state[i]), abs(stage[i])))))
        error_sum = (error_sum + (scaled_error * scaled_error))
    return sqrt((error_sum / 4.0))
```

</details>

<a id="function-16"></a>
### @metablate.tangent_discriminant

Called by: [@metablate.tangent_zenith](#function-17). Calls: [@metablate.geometry](#function-9).

<details>
<summary>View implementation</summary>

```text
function @metablate.tangent_discriminant(cfg: Array<f64,1>, zenith: f64) -> f64:
    var geometry = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    geometry(cfg, zenith, geometry)
    let pp = ((((geometry[0] * geometry[0]) / 40680631590769.0) + ((geometry[1] * geometry[1]) / 40680631590769.0)) + ((geometry[2] * geometry[2]) / 40408299984661.445))
    let pu = ((((geometry[0] * geometry[3]) / 40680631590769.0) + ((geometry[1] * geometry[4]) / 40680631590769.0)) + ((geometry[2] * geometry[5]) / 40408299984661.445))
    let uu = ((((geometry[3] * geometry[3]) / 40680631590769.0) + ((geometry[4] * geometry[4]) / 40680631590769.0)) + ((geometry[5] * geometry[5]) / 40408299984661.445))
    return ((pu * pu) - (uu * (pp - 1.0)))
```

</details>

<a id="function-17"></a>
### @metablate.tangent_zenith

Called by: None. Calls: [@metablate.tangent_discriminant](#function-16).

<details>
<summary>View implementation</summary>

```text
function @metablate.tangent_zenith(cfg: Array<f64,1>) -> f64:
    var low = 60.0
    var high = 89.0
    for i in range(0, 50):
        let mid = (0.5 * (low + high))
        let discriminant = tangent_discriminant(cfg, mid)
        if (discriminant > 0.0):
            low = mid
        if (discriminant <= 0.0):
            high = mid
    return (0.5 * (low + high))
```

</details>

<a id="function-18"></a>
### @metablate.temperature_rate

Called by: [@metablate.temperature_rate_si](#function-19). Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.temperature_rate(mass: f64, velocity: f64, temperature: f64, rho_m: f64, cp: f64, latent: f64, shape: f64, rho_air: f64, mass_loss_positive: f64, heat_transfer: f64, ambient: f64, emissivity: f64) -> f64:
    let flux = (((((((0.5 * heat_transfer) * rho_air) * velocity) * velocity) * velocity) - ((2.2681497676737726e-07 * emissivity) * ((((temperature * temperature) * temperature) * temperature) - (((ambient * ambient) * ambient) * ambient)))) - (((latent / shape) * pow((rho_m / mass), 0.6666666666666666)) * mass_loss_positive))
    return ((shape * flux) / ((cp * pow(mass, 0.3333333333333333)) * pow(rho_m, 0.6666666666666666)))
```

</details>

<a id="function-19"></a>
### @metablate.temperature_rate_si

Called by: [@metablate.rhs](#function-14). Calls: [@metablate.temperature_rate](#function-18).

<details>
<summary>View implementation</summary>

```text
function @metablate.temperature_rate_si(mass: f64[kg], velocity: f64[m/s], temperature: f64[K], rho_m: f64[kg/m^3], cp: f64[J/kg/K], latent: f64[J/kg], shape: f64, rho_air: f64[kg/m^3], mass_loss_positive: f64[kg/s], heat_transfer: f64, ambient: f64[K], emissivity: f64) -> f64[K/s]:
    return (temperature_rate((mass / 1.0 [kg]), (velocity / 1.0 [m/s]), (temperature / 1.0 [K]), (rho_m / 1.0 [kg/m^3]), (cp / 1.0 [J/kg/K]), (latent / 1.0 [J/kg]), shape, (rho_air / 1.0 [kg/m^3]), (mass_loss_positive / 1.0 [kg/s]), heat_transfer, (ambient / 1.0 [K]), emissivity) * 1.0 [K/s])
```

</details>

<a id="function-20"></a>
### @metablate.thermal_mass_loss

Called by: [@metablate.thermal_mass_loss_si](#function-21). Calls: None.

<details>
<summary>View implementation</summary>

```text
function @metablate.thermal_mass_loss(mass: f64, temperature: f64, rho_m: f64, mu: f64, ca: f64, cb: f64, shape: f64) -> f64:
    let vapor_pressure = (0.1 * pow(10.0, (ca - (cb / temperature))))
    return ((((-4.0 * shape) * pow((mass / rho_m), 0.6666666666666666)) * vapor_pressure) * sqrt((mu / (8.67487351117219e-23 * temperature))))
```

</details>

<a id="function-21"></a>
### @metablate.thermal_mass_loss_si

Called by: [@metablate.rhs](#function-14). Calls: [@metablate.thermal_mass_loss](#function-20).

<details>
<summary>View implementation</summary>

```text
function @metablate.thermal_mass_loss_si(mass: f64[kg], temperature: f64[K], rho_m: f64[kg/m^3], mu: f64[kg], ca: f64, cb: f64[K], shape: f64) -> f64[kg/s]:
    return (thermal_mass_loss((mass / 1.0 [kg]), (temperature / 1.0 [K]), (rho_m / 1.0 [kg/m^3]), (mu / 1.0 [kg]), ca, (cb / 1.0 [K]), shape) * 1.0 [kg/s])
```

</details>
