# Physics fixes: lava/water, lava/wood, fire/oil

Status: plan. Nothing implemented yet. Source ideas: the ChatGPT export at `/Users/anujbarve/Documents/MYDATA/chatgpt/202611042040-chatgpt-ChatGPT-12msg.md`.

## 1. Problems (as reported in the viewer)

1. Lava next to water does not evaporate the water.
2. Wood next to lava does not catch fire.
3. Oil next to fire does not catch fire.

Current tests pass for a related case (`lava_ignites_wood`: wood burns within 10 s, `lava_heats_water`: water gets above 60°C), so the tests are too weak, not evidence that the physics works. The viewer case has not been reproduced yet. Step 1 of the plan does that.

## 2. What the export says, and what we keep

The export is a brainstorm: 15 systems and about 300 properties. It is not a spec. For these three bugs, only four ideas matter:

- **Coupled chains.** Heat drives phase change; phase change releases or absorbs energy; that energy drives pressure and motion. The three bugs are all broken links in such chains.
- **Latent heat.** Boiling and melting absorb energy at a fixed temperature. The export lists it; our model skips it. This is the main cause of bug 1.
- **Separate layers for heavy physics.** Keep the cell small, activate expensive solvers only where needed. Relevant if we later add a pressure field.
- **Shared material table.** Already the case (`COND`, `CAP`, `DENSITY`, ...).

Deferred (not needed for these bugs): electricity, magnetism, chemistry, biology, fracture, optics beyond what exists, multiphase fluid solvers.

## 3. Root causes (hypotheses, to verify in step 2)

| bug | likely cause in current code |
|---|---|
| lava -> water | Heat only crosses the water/lava edge by conduction and radiation between two cells. Water (heat capacity 4180 kJ/m³K) takes a long time to reach 100°C. No latent heat, so boiling is a threshold with no energy cost. Steam and convection are slow to carry heat away. |
| lava -> wood | Wood reaches 300°C by conduction from a 1150°C neighbour. Wood's own heat capacity and conductivity make the front move slowly, and the fire start (`life 120+`) is set only in the wood cell that crosses 300°C. Lava may cool before the front reaches the wood. |
| fire -> oil | Fire cells live 4 s and radiate to other materials at 1/3 strength. Oil ignites at 250°C, and fire is only held at 900°C while `life > 0`. Oil next to fire has a short window. Ignition also needs the oil cell itself to be hot, with no separate "fuel reached flame" rule. |

Common thread: heat transfer at a material boundary is only conduction between two cell centres. Real contacts have a boundary layer, a radiation view factor, and latent energy. The model has none of these.

## 4. Plan

### Step 1: reproduce, with numbers (1 day)

- Add three scenes to the test bench (`physics/mod.rs`): `lava_on_water` (a 40-cell lava pool on a water bed), `lava_wood_wall` (a lava column against a wood wall), `fire_oil_tub` (fire at one end of an oil tub).
- Metrics, logged per 0.1 s of world time:
  - hottest water temperature and steam cell count (lava/water),
  - wood cells on fire and the time of first ignition (lava/wood),
  - oil cells on fire and the time of first ignition (fire/oil).
- Write the failing tests first with the target numbers from section 5. Each test runs in CI-style time (under 5 s wall time).
- Add a viewer-independent dump (`SAND_TRACE=1`) that prints the metrics, so the numbers can be checked against what is seen on screen.

Exit: the three tests fail, and the metrics match what the viewer shows. If the viewer shows something the tests do not, fix the scene before the physics.

### Step 2: instrument heat flux (1 day)

- Per-step counters on the CPU reference (`cpu.rs`): heat moved across each material pair (water-lava, wood-lava, fire-oil), and energy stored by each material.
- Answer, with numbers: how much energy reaches the water per second; how much of that goes into raising its temperature versus being lost; how long the wood front takes to reach 300°C.

Exit: a table of energy per second per boundary, and which term is limiting in each bug.

### Step 3: boundary contacts (2 days)

Replace the single-edge model at material boundaries with a contact term:

- **Contact conductance** between two different materials: a boundary coefficient $h$ (W/m²K) independent of the two cells' conductivities. Use $h \approx 300$ to $1000$ W/m²K for liquid-solid, $\approx 20$ to $50$ for gas-solid. Converted to a per-step rate with the same scale as `RATE_SCALE`.
- **Radiation view factor**: a surface sees the emitters around it. Instead of 1/3 of contact radiation, a flame or lava cell radiates to every non-gas neighbour within 3 cells, with a falloff $\propto 1/d^2$ and factor $\varepsilon\sigma T^4$ (already in `radiation_k`, but only nearest neighbours today).
- Keep the same stochastic rounding and antisymmetry, so energy stays conserved (the energy test already checks this).

Exit: the step 1 metrics move in the right direction. Heat conservation test still passes.

### Step 4: latent heat and phase energy (2 days)

- Each phase change has a latent-energy budget: water to steam absorbs $L_v = 2.26$ MJ/kg, ice to water $L_f = 0.33$ MJ/kg, lava to stone releases about $0.4$ MJ/kg.
- Model it as an enthalpy field per cell: temperature is only allowed to move between the transition temperatures while the enthalpy is consumed or released. Store enthalpy in the packed cell (we have 20 bits of temperature; reuse them as enthalpy offset from the phase), or in a second u32 buffer if the bits are not enough.
- Effect: water next to lava stays at 100°C while it boils. Lava next to water loses energy fast and stays hot longer.

Exit: `lava_on_water` produces steam at a rate that matches the energy budget within 20%. A test checks the energy balance (heat in = sensible heat + latent heat + losses).

### Step 5: fuel and burning (2 days)

- Ignition: a fuel cell ignites when its temperature passes its ignition point. Points: wood 300°C, oil 250°C (flash point), fire contact keeps the fuel above the ignition point until the fuel is burned.
- Burning: fire stops being a fixed 900°C with a 4 s timer. Instead, burning fuel releases heat at a rate $\dot Q$ (wood 16 MJ/kg, oil 42 MJ/kg) while fuel mass decreases. Fire lasts as long as the fuel lasts; flames are the burning cell, not a separate life counter.
- Fire spread: a fuel cell touching fire, or within radiation range of it, ignites when its temperature exceeds the ignition point. This is the step 3 radiation, so step 5 and step 3 must land together.

Exit: `fire_oil_tub` ignites oil within 2 s of contact, and the fire spreads through the oil at a rate that matches the step 1 numbers within 30%.

### Step 6: check and tune (1 day)

- All step 1 tests pass, plus the existing tests (conservation, sleeping, GPU/CPU bit-exact).
- Bench: GPU cost per step must not rise by more than 25%. The contact and radiation terms add neighbour reads; fold them into the existing fast paths.
- Viewer check: the three scenes, screenshot each, compared against the expected behaviour in section 5.

## 5. Targets (to be set in step 1, then fixed)

| scene | target |
|---|---|
| lava on water (40-cell pool, 1150°C) | water beside lava reaches 100°C within 2 s; steam produced within 3 s; some lava hardens within 10 s |
| lava against wood (column, 1150°C) | first wood cell ignites within 3 s |
| fire against oil (one fire cell) | oil ignites within 2 s; the tub is fully burning within 15 s |

These are for a 5 mm cell and real material properties. If a target needs a number that does not exist in the material table, change the table and record why in the commit message.

## 6. Risks

- **Performance.** Radiation over a 3-cell radius costs about 50 reads per cell. Mitigation: only for cells adjacent to a hot cell (chunk sleeping already tracks activity) and only in chunks that have a fuel or lava cell.
- **Enthalpy storage.** The packed cell has 20 bits for temperature. Enthalpy may not fit. Fallback: second buffer, which adds memory traffic (currently about 32 bytes per step per cell; a second buffer roughly doubles it).
- **Tuning.** Real numbers may give scenes that look too slow at 1x. The world clock already handles this: 16x speed for watching, real numbers stay in the model. If it still feels wrong, the trade-off is physical accuracy versus time to see results, and that is a decision for you.
- **Tests that pass for the wrong reason.** The existing wood and water tests passed while the viewer failed. Step 1 must compare against what is seen on screen, not only against the test scene.

## 7. Scope check against the export

The export asks about 300 properties per cell. This plan adds: contact boundary, radiation view factor, latent heat, fuel and burn rate. About 4 new quantities per cell, all in the thermal and combustion systems. Pressure, humidity, wind and the rest of the export list stay out until these bugs are fixed and tested.

## 8. Order of work

1. Step 1 (reproduce, failing tests). Stop here if the reproduction differs from the viewer.
2. Steps 2 to 5 in order. Each one is a separate commit with its tests.
3. Step 6.

Nothing in this plan changes the audio plan (`docs/plans/audio.md`). Audio can start after step 4, since latent heat events (boil, solidify, quench) are what it needs to sound right.
