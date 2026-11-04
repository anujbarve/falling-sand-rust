# Property triage: the 286 properties in the ChatGPT export

Source: the export's 15 sections (plus the 12-item "core properties" table). Judgement calls, not measurements. Each property goes to one bucket:

- **State**: stored per cell, changes every step. Costs memory and bandwidth.
- **Constant**: one value per material, in the shared table. Costs nothing per cell.
- **Field**: one value per grid region or per frame (wind, ambient pressure, gravity). Costs almost nothing.
- **Derived**: computed from state and constants when needed. Costs time, not memory.
- **Later**: sensible, but not needed for the current bugs or the audio plan.
- **Cut**: not worth it at this scale or not a fit for a 2D falling-sand sandbox.

## 1. Totals

| bucket | count (approx.) | notes |
|---|---|---|
| State | 12 | per cell |
| Constant | 95 | shared material table |
| Field | 10 | per grid or per frame |
| Derived | 80 | computed on demand |
| Later | 40 | after the physics fixes |
| Cut | 49 | out of scope |

About 25 items appear twice in the export (for example "Deposition" in sections 4, 12 and 13; "Cohesion" in 2 and 5; "Permeability" twice in 13). They are counted once above.

Conclusion: **only about 12 values per cell need storing.** The export's 44-variable "in-depth" cell is mostly constants and derived values stored by mistake.

## 2. The 12 state values per cell

| value | why per cell | size |
|---|---|---|
| material id | which substance | 4 bits (already) |
| temperature | heat (already) | 20 bits (already) |
| enthalpy / phase energy | latent heat (physics-fixes step 4) | 16 bits, or a second u32 |
| velocity x | motion | i16, fixed point |
| velocity y | motion (already as speed level, 8 bits) | i16 |
| fuel / life | burning and fire lifetime (already as aux, 8 bits) | 8 bits |
| moisture | wet sand, humidity sources | 8 bits |
| oxygen | combustion | 8 bits |
| water vapour | humidity, condensation | 8 bits |
| smoke | smoke density | 8 bits |
| damage | fracture (later) | 8 bits |
| charge | electricity (cut, see below) | 8 bits, reserved |

Total with current packing: about 16 bytes per cell. Today it is 4. At 640x360, 16 bytes is four times the memory traffic of now. The current step is 0.12 ms at that size; four times traffic should still leave real-time headroom at 640x360, but not at 1080p. That is a limit to measure, not assume.

## 3. Section by section

### 1. Fundamental physics (20 items)
- State: velocity x/y.
- Constant: mass per cell (from density and cell volume), restitution, friction (static and kinetic).
- Derived: acceleration, momentum, inertia, force, drag, buoyancy, terminal velocity, impulse transfer.
- Field: gravity.
- Later: elasticity (bounce, needs a collision model).
- Conservation of mass, momentum and energy: properties of the update rules, not stored values. The conservation tests already cover heat; add mass and momentum tests.

### 2. Fluid dynamics (22 items)
- Constant: viscosity, surface tension, cohesion, adhesion, compressibility.
- Field: hydraulic transmission (through the pressure field, later).
- Derived: hydrostatic pressure (column weight), dynamic pressure, flow rate, displacement.
- Later: pressure field (see section 5 below), turbulence, vorticity, wave propagation.
- Cut: laminar flow, cavitation (needs a pressure solver to matter).

### 3. Gas and atmosphere (22 items)
- State: water vapour, oxygen, smoke, gas temperature (already).
- Constant: gas viscosity, gas permeability, molar mass per gas.
- Field: atmospheric pressure, wind speed and direction, humidity baseline.
- Derived: air density (already, ideal gas), relative humidity, dew point, vapour pressure (Antoine equation from temperature), buoyancy, convection.
- Later: pressure waves, shock waves.
- Cut: carbon dioxide (one concentration is enough), gas diffusion as a separate process (cover with a diffusion step on the concentration fields).

### 4. Thermodynamics (23 items)
- State: temperature (have), enthalpy (new).
- Constant: heat capacity, conductivity, diffusivity, expansion coefficient, emissivity, melting, freezing, boiling, condensation points, latent heats.
- Derived: thermal diffusivity (conductivity divided by density times heat capacity), contraction (opposite of expansion), evaporation rate (from vapour pressure and wind), equilibrium, pressure-dependent boiling point.
- Later: sublimation, deposition (the same mechanism as condensation and freezing, one table entry each).

This section is the core of the physics fixes. Most items are already in the material table. The missing piece is enthalpy.

### 5. Granular (21 items)
- Constant: angle of repose, friction, cohesion (dry), particle size, packing density.
- State: moisture (wet sand clumps more: moisture modifies the constant).
- Derived: granular pressure, shear resistance, avalanche condition (slope against angle of repose), jamming.
- Later: granular pressure in a real pressure field, compaction, sedimentation.
- Cut: percolation, arch formation, grain-size distribution (needs per-grain size; out of scope for cells), fluidisation (interesting, but expensive).

Angle of repose currently emerges from the random diagonal rule. Make it an explicit constant and check that the pile angle matches it. Good test.

### 6. Solid mechanics (21 items)
- Constant: hardness, strengths (tensile, compressive, shear, yield), toughness, ductility, brittleness, impact resistance, expansion.
- State: damage (only if fracture is added).
- Derived: stress, strain, thermal stress, load distribution.
- Later: fracture propagation, cracking, buckling, fatigue, structural integrity. Each needs a stress field. Do only after the fire and heat fixes.
- Cut: full elastic and plastic deformation tensors. Solids in the sim do not deform: they are cells.

### 7. Combustion and fire (20 items)
- Constant: ignition temperature, flash point, autoignition temperature, heat release rate, soot yield, combustion efficiency, oxygen requirement.
- State: fuel (have as life, will become mass), oxygen (state), smoke (state).
- Derived: flame temperature (from heat release and gas heat capacity), flame propagation (from radiation and ignition rules).
- Later: smouldering, pyrolysis as its own stage (wood gives off gas before it burns).
- Cut: deflagration and explosion pressure waves (out of scope without a pressure solver).

Fuel and burning are physics-fixes step 5. The items here map straight onto that plan.

### 8. Chemistry (20 items)
- Constant: reactivity, activation energy, enthalpy, solubility, saturation, pH.
- Derived: reaction rate (Arrhenius from temperature), equilibrium.
- Later: corrosion, dissolution, precipitation, crystallisation. Only if they are needed for a specific material pair (acid on stone is already in the sim as a hand-made rule).
- Cut: catalysis, reaction diffusion, chemical decomposition, material compatibility, concentration as a general field. Each would need its own reaction table; that is a second game.

The current acid rule is a hand-written reaction. Keep that approach for new reactions. Do not build a general chemistry engine.

### 9. Electricity and magnetism (19 items)
- Cut, all of them, for now. Nothing in the material set conducts, and no tool is planned for it.
- Keep one thing: Joule heating (current times resistance) if a conductive material is ever added. It is a heat source, nothing more.

### 10. Light and optics (18 items)
- Constant: emissivity, absorption, refractive index, opacity, translucency.
- Derived: blackbody glow (already in the renderer), incandescence (already), glow intensity, shadow casting (from opacity).
- Later: transparency, refraction (needs a per-pixel ray, which the renderer could do cheaply), caustics.
- Cut: Fresnel, dispersion, volumetric lighting, scattering beyond a simple fog term. The renderer already does bloom and lighting: this is the cheap part. Real optics is out.

### 11. Acoustics (17 items)
- Constant: acoustic impedance, sound absorption, damping, resonance frequency (per material, used by the audio plan's modal sets).
- Derived: granular collision sounds, bubbling (Minnaert), crackling. These belong to the audio plan, not the physics.
- Cut: Doppler effect, sound transmission through matter. The audio plan does not model propagation.

### 12. Phase transitions (16 items)
- Constant: all transition points and latent heats (already in section 4).
- Derived: supercooling and superheating (a nucleation probability above and below the transition point), evaporative cooling (from latent heat), phase equilibrium.
- Later: nucleation (random per cell, gives realistic delayed boiling and freezing).
- Cut: boiling under reduced pressure (needs the pressure field).

### 13. Environment and geology (17 items)
- Constant: porosity, permeability, soil saturation limit, freeze-thaw temperature.
- State: moisture (have).
- Derived: infiltration, groundwater flow, compaction.
- Later: erosion, sediment transport, landslides, liquefaction. A separate game's worth of rules; add one of them only if a scene needs it.
- Cut: wind erosion (wind is a field, and this would double the cost), material layering (the grid is the layering).

### 14. Biology (12 items)
- Cut, almost all. Keep moisture, which section 13 already covers.
- Later, optionally: decay of organic material (a slow fuel-to-soil transition, one rule, one table entry).
- Cut: osmosis, microbial activity, mould, nutrient diffusion, oxygen consumption by life. Nothing in the sandbox is alive.

### 15. Advanced physics (18 items)
- Cut, all, except:
- Later: non-Newtonian behaviour (shear thinning or thickening, as a flag in the viscosity table). Cheap, and it makes oil and lava look different in motion.
- Cut: magnetohydrodynamics, electrohydrodynamics, Marangoni, Rayleigh-Taylor, Kelvin-Helmholtz and the other instabilities. They emerge from solvers at much higher resolution than 640x360. Do not chase them.

### Core properties table (12 items)
Material id, temperature, velocity x/y, mass or fill, pressure, density, moisture, phase, internal energy, concentration, charge, damage. All covered in section 2 above. Fill fraction is a useful addition: partially filled cells would fix the "one grain of water stuck in a cell" look at surfaces (later).

## 4. Memory and cost, with the buckets applied

| version | per cell | 640x360 memory | notes |
|---|---|---|---|
| now | 4 B | 0.9 MB per buffer | |
| plan (state values above) | about 16 B | 3.7 MB per buffer | fits, traffic 4x |
| export "in-depth" (44 values) | 128 B | 29 MB per buffer | traffic 32x; unworkable at real time |

Memory is not the problem. Bandwidth is: every state value is read and written every step in every cell. The triage keeps state small; constants and derived values cost nothing per cell.

## 5. Field-level items: one solver each, not per cell

- **Pressure**: one scalar per cell, solved from the fluid column and gas heating. Later, not needed for the three current bugs. Expensive: needs a solve per step (Jacobi or multigrid iterations). Do it only if steam or gas pressure must push things.
- **Wind**: one vector per chunk, driven by temperature differences (buoyancy). Cheap.
- **Ambient pressure and humidity**: one value for the whole world. Trivial.
- **Gravity**: constant, already.

## 6. What this changes in the plan

- `physics-fixes.md` stays as the plan for the three bugs. It needs enthalpy (state), contact conductance (constant plus derived), radiation (derived), and fuel (state: fuel and oxygen). That is 3 of the 12 state values.
- Add the rest only when a scene needs them. Each new state value is a new bandwidth cost; each derived value is a new CPU cost. Adding either needs a bench before and after.
- Constants: move everything in sections 1 to 4 and 7 into the material table now. Most already exist; the table can be a single struct per material, which also fixes the duplicated values in `sim.wgsl`.

## 7. Open question

Is a pressure field in scope? If yes, it is the single most expensive addition, and it is also what makes steam push sand and water push out of a sealed container. It needs its own plan. Not decided here.
