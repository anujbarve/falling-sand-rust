# Game design: calm ecological sandbox (draft for approval)

Status: draft, revision 2. Nothing is built until the user approves this document.
Companion: `2026-11-06-art-style.md` (look and palette).

Items marked **[my call]** are decisions made here without the user's explicit answer. They can be changed at approval. Items marked **[open]** need the user's answer before the plan can be finished. Items marked **[rev 2]** were changed in revision 2 to remove a contradiction or fill a gap; the reason is given in place and summarised in section 0.

---

## 0. Revision 2 changes

Revision 1 had contradictions that would have blocked the build, and several missing mechanisms. Each change below is detailed in the section it points to.

| # | Problem in rev 1 | Fix in rev 2 | Section |
|---|---|---|---|
| 1 | Zoom changed view extent while cell size stayed fixed: impossible on a fixed window. | No zoom. View extent = window size. The budget caps the view, not a zoom level. | 3.3, 5.4 |
| 2 | "Simulate chunks that overlap the view" meant up to 63 cells of margin, not 16. | Chunks are storage; simulation is clipped to the active rectangle inside each chunk. | 5.1 |
| 3 | 64x speed meant 64 physics steps per frame: unaffordable. | 20 TPS base tick rate; integer tick clock; random ticks for slow processes with a tick multiplier above the per-frame cap. | 5.3, 6.1, 7.4 |
| 4 | "No sky test" contradicted exposure, snow, erosion, growth. | Per-column surface heightmap makes exposure an O(1) lookup. | 4.5 |
| 5 | Cloud band, rain and ambient temperature depended on the camera. | Ambient is a function of world position and time; rain and the water cycle are driven by the weather field, not the view. | 5.7, 6.2, 9 |
| 6 | Local support rule let floating roofs stand. | Connected-group support check, run only on change. Unsupported groups fall as rigid bodies. | 8.7 |
| 7 | Water had no sink; moisture never decreased. | Evaporation, drying, soak-then-runoff, and a regional humidity pool. | 8.8 |
| 8 | The world only decayed. | Tree regrowth, leaf cycle, soil formation; erosion tuned slow. | 7.4 |
| 9 | Generated terrain was not at rest; lakes needed non-local data. | Generation rules guarantee rest; lakes use bounded feature placement. | 4.3 |
| 10 | Tides and waves broke volume conservation. | No tides. Waves are a rendered surface effect in v1; shore erosion moves to v2. | 4.3 |
| 11 | Explosions threw material with no velocity model. | Particle list with its own budget. | 8.5 |
| 12 | No multi-threaded update rule or determinism. | Four-phase chunk update, alternating scan, hashed RNG. | 5.8 |
| 13 | 8 bytes per cell did not fit the listed fields. | Explicit 8-byte layout; air has no stored temperature. | 5.4 |
| 14 | Chunks were never unloaded. | Resident chunk cache with an LRU rule. | 4.2 |
| 15 | Watchdog froze parts of the screen. | Watchdog slows the whole world uniformly instead. | 5.4 |
| 16-18 | Undefined materials, no plant movement rule, "thinner" metal. | Material table completed; plant kind defined; rust removes whole cells. | 7, 8.1 |
| 19-22 | No zoom key (now moot), no tool selection, undefined undo and paint rules. | Bar pages, Q/E, local-revert undo, paint rules. | 3.2, 10 |
| 23 | Window assumed 16:9 at fixed sizes. | Any window size and DPI; budget per machine in cells. | 3.3, 5.4 |
| 24 | "Region" meant three things. | Terms defined: active region, weather column. | 4.6 |
| 25-27 | Save state incomplete, no generator version, weak size claim. | Full saved state, generator version, compressed region files. | 13 |
| 28-29 | Milestone order had tests before their features. | Milestones reordered. | 15, 16 |
| 30-31 | Unverified hardware and audio assumptions. | Pinned as M0/M5 checks and open questions. | 5.5, 11, 18 |
| 32 | Open questions out of date. | Rewritten. | 18 |

**Second-pass fixes (rev 2.1).** A re-read of rev 2 found problems, mostly introduced by the rev 2 fixes themselves:

| # | Problem in rev 2 | Fix | Section |
|---|---|---|---|
| 33 | Pillar 6 ("camera never changes physics") was overclaimed: the margin wall and gas absorption depend on the camera by design. | Pillar and test scoped to cells away from the margin. | 2, 14.1 |
| 34 | Steam condensed at an absolute altitude, so steam from a mountain and from the shore behaved differently, and the view might not reach it. | Steam cools by itself and condenses a short, fixed distance above its source. | 5.7, 8.2 |
| 35 | Only the top cell of a column exchanged heat with air: lava in a cave or a hot wall's side face never cooled. | Two notions: sky-exposed and air-exposed. Any air neighbour exchanges heat. | 4.5, 6.2, 8.2 |
| 36 | Falling raindrops and sand counted as the surface, so the heightmap changed every step and the ground flickered between sheltered and exposed. Leaves made everything under a tree "sheltered". | The heightmap counts settled cells only, and ignores plants. | 4.5 |
| 37 | Rain had no rule for hitting grass or leaves. | Plants absorb water and drip when saturated. | 8.8 |
| 38 | Ice floes and wooden rafts would sink, because liquid did not count as support. | Groups float on denser liquids. Group moves run outside the parallel phases. | 8.7, 5.8 |
| 39 | A 32-cell move plus its destination check could reach 33 cells and break the four-phase guarantee. | Maximum move is 31 cells. | 5.8, 8.1 |
| 40 | Slow-process visits were scheduled per frame, so results depended on frame timing; the determinism test would fail. | Replaced stripe visits with per-tick deterministic random ticks (`randomTickSpeed`). | 5.8, 7.4, 14.1 |
| 41 | Weather values jumped at 512-cell column borders: a visible rain edge and temperature step. | Values interpolated between column centres. | 9.1 |
| 42 | Rain-or-snow was decided at the spawn height, which is the top of the view, so the camera decided precipitation. | Decided by ambient at the column's surface. | 9.1, 9.2 |
| 43 | Generated lakes on dry soil would soak away on first activation, breaking the rest guarantee. Long-run water levels could drift. | Soil under generated water starts saturated; seepage stops at stone; drift tested. | 4.3, 14.3, 17 |
| 44 | Eviction and saving could both write the same region file. | One writer thread owns all disk writes. | 13 |
| 45 | M0 had a cache that evicts changed chunks to disk, but saving arrives in M4. | Until M4, changed chunks are never evicted. | 4.2, 15 |
| 46 | A burning wall turned into fire cells (gas-like), so it lost its support the instant it caught, and burning houses collapsed in a frame. | Burning is a flag on the cell; flames are separate gas cells. | 5.4, 7.2, 8.3 |
| 47 | Grass buried under sand or snow lived forever. | Grass that stays out of light dies back to soil. | 7.4 |
| 48 | 1 cell/tick fall speed at 20 TPS was 3x slower than 60 TPS (13.5 s across 1080p), breaking §8.4 time limits. | Gravity moves falling powders, liquids and rigid groups up to 3 cells/tick (60 cells/s); gases rise 2 cells/tick. | 5.7, 8.1, 8.7 |
| 49 | `randomTickSpeed = 3` had high Poisson variance for single-cell 1-day tests. | Raised default to $K = 16$ per chunk per tick; slow-process tests evaluate a 16-cell strip majority. | 7.4, 14.1 |
| 50 | Simulated falling rain cells clashed visually with Art Style §8's 1,400 decorative rain streaks and depended on top-of-view height. | Decorative streaks show falling rain; simulation applies rain/snow directly at each column's surface via the heightmap. | 9.2, 14.1 |
| 51 | Sub-cell art features (1-art-px crest rims, grass tufts) need a 2x2 art-pixel buffer per cell, and the 21 v1 materials lacked hex colours. | Specified the 2x2-per-cell art-pixel framebuffer and added the 21-material palette table. | 3.3, 5.6, 7.2 |
| 52 | §5.3 claimed "one tick every 3 frames" tripled the per-tick CPU budget, and left fractional tick multipliers, $M \times p > 1$, high-$M$ visit drop, and multi-seed test reliability unspecified. | Elapsed-time tick scheduling with a single-tick frame budget ($\le 10\text{ ms}$) and per-frame total simulation cap ($\le 12\text{ ms}$); overdue physics tick discard + pause/speed reset; Bresenham remainder for exact `world_tick`; random-tick count scales as $\min(K \times M, 256)$ with per-tick slow-parity bit and stage-clamped $1-(1-p)^m$ fallback; multi-seed + 1x/16x/64x test suite. | 5.3, 5.4, 7.4, 14.1 |

---

## 1. Intent

A sandbox world with no character, no monsters, no score and no goal. The player is an observer. They build or place things in a living world, then watch the world act on them: rain soaks the wood, fire spreads through dry grass, water finds the low ground, seasons change the hill.

Core loop: **place, step back, watch, place again.** The reward is watching cause and effect work, not finishing a task.

The first finished goal, and the first video, is one scene: a house on a hillside weathering through rain, fire and seasons.

Targets:
- Runs well on a first-generation Core i3 (2010, for example i3-530 or i3-330M) with 2-4 GB RAM, and on modern Mac (Apple silicon) and PC. [rev 2: the first Core i3 shipped in January 2010; the exact test machine is open question 1.]
- Looks like the approved art style, so a 60-second clip works as a video opener.
- Small enough to finish solo, and clear enough to teach in a Rust series.

Non-goals:
- No combat, enemies, quests, levels, unlocks, or progression.
- No fixed map. The world is infinite and procedurally generated.
- No accurate science simulation. Physics is "behaves as expected," not "matches real numbers."
- No multiplayer.
- No 3D.
- No camera zoom. [rev 2, see 3.3]
- No tides. [rev 2, see 4.3]

Anti-goals (would count as failure even if technically working):
- Physics that is correct on paper but looks broken to a viewer (for example: lava next to water that does not boil, because the boil takes too long to see).
- Frame rate collapse on the target hardware.
- Visuals that fight the art style: noise, blur, glowing UI, arcade colours.
- Any mechanic that needs reading a manual to notice.
- Any part of the visible screen frozen while the rest moves.

---

## 2. Design pillars

1. **Cause and effect you can see.** Every reaction must be visible within seconds, and the cause must be easy to read from the screen.
2. **The world is the main character.** Each frame is beautiful with no input.
3. **Light and weather carry the mood.** Atmosphere comes from the time of day, rain, wind and season, not from UI or effects.
4. **Cheap by default.** Only what the viewer can see is simulated. Everything else is stored, not run.
5. **Simple rules, consistent results.** A few material rules that combine into many visible results.
6. **The camera never changes the physics.** [rev 2, scoped in 2.1] Where the camera is decides *which* cells run, never *how* they behave. Temperature, rain or snow, the snow line and where steam condenses depend on world position and time only. The one deliberate exception is the margin itself (section 5.7): material stops or is absorbed there, so a cell's behaviour can depend on the camera only within reach of the margin.

---

## 3. Player experience

### 3.1 The first minute
The game opens on a generated landscape at the start of a day. There is no tutorial text. A small material bar sits at the bottom of the screen. The player picks a material and paints. The world responds immediately: sand slides, water runs downhill, a stone block stays put.

### 3.2 Controls [my call]
- **Mouse left:** paint the selected material or use the selected tool. Hold to keep painting.
- **Mouse right:** erase (paint air).
- **Alt + mouse left:** paint into empty (air) cells only, leaving existing material alone. [rev 2]
- **Mouse wheel / [ and ]:** brush size, 1 to 24 cells.
- **Number keys 1-9, 0:** select a slot on the current bar page.
- **Q / E:** previous / next bar page (Materials 1, Materials 2, Tools). [rev 2: 19 materials and 5 tools do not fit on ten keys.]
- **Shift + drag:** line brush.
- **Middle drag / WASD:** pan the camera.
- **Space:** pause or resume time.
- **, and .:** slow time / speed time (0.25x, 0.5x, 1x, 4x, 16x, 64x).
- **Ctrl+Z / Cmd+Z:** undo (section 10).
- **Tab:** toggle the material bar.
- **M:** mute.
- **Escape:** settings (graphics, sound, save, quit).
- **F5 / F9:** save and load.

### 3.3 Camera and view size [rev 2, replaces the zoom rule of 2026-11-06]
- Free camera, no snapping. Smooth easing, no shake.
- **A cell is always 4 physical screen pixels** (2 x 2 art pixels, see the art style doc, section 1). The renderer writes to an **art-pixel buffer** (2 x 2 art pixels per cell, scaled 2x nearest-neighbour to physical pixels), so 1-art-pixel crest rims and 1-2-art-pixel grass tufts are drawn cleanly inside a cell's 2 x 2 block. [rev 2.2]
- **There is no zoom.** Rev 1 said zoom changes how much world the view covers while cells stay 4 px. On a fixed window that is impossible: if cell size and window size are both fixed, the cell count on screen is fixed. Covering fewer cells means drawing them bigger, which the art style forbids. The only way to see more world is a bigger window, and that is what resolution already does.
- **View extent = window size in physical pixels / 4**, for any window size, aspect ratio or DPI. The window is resizable. On macOS the window is created high-DPI, so a Retina screen uses physical pixels and shows more world.
- Cells on screen by window: 1280 x 720 = 320 x 180 (57,600); 1920 x 1080 = 480 x 270 (129,600); 2560 x 1440 = 640 x 360 (230,400); MacBook Pro 14" fullscreen 3024 x 1964 = 756 x 491 (371,196).
- **The machine caps the view, not the design.** If a window would hold more active cells than the machine's budget (section 5.4), the view is capped at the largest centred rectangle that fits the budget, keeping the window's aspect ratio. The rest of the window is filled with a plain panel in the UI colour. Nothing outside the rectangle is drawn as world, so there is never a frozen strip of world on screen.
- Camera position is saved with the world.
- If zoom is wanted later, one of two rules must break: either cells may be drawn larger (integer scale, nearest-neighbour), or the art grain changes. That is open question 2.

### 3.4 What the player does (there are no quests)
- Builds shelters, dams, paths, fire breaks, bridges.
- Starts fires, floods, storms (through tools in section 9). Eruptions come with volcanoes in v2.
- Waits for seasons.
- Explores by panning across generated terrain.

Lamps are removed from v1. [rev 2: there is no light-emission system to make them work. They return when lighting from sources is designed, after M5.]

---

## 4. The world

### 4.1 Coordinates and scale [my call]
- One **cell** is the simulation unit. The world is measured in cells and is infinite horizontally. Vertically it is bounded: generation fills from a fixed bedrock depth to a fixed sky ceiling (both far outside normal play; values set in M4). [rev 2: an infinite vertical world would make the surface heightmap and generator unbounded for no visible gain.]
- y increases upward in this document. "Altitude" means world y.
- A cell is an abstract unit, not a fixed physical length. [open] A physical scale (1 cell = 1 cm or similar) is a tuning decision for section 8, because it sets how fast falling material reads. It is deliberately not fixed here. (Open question 9.)

### 4.2 Chunks [my call, rev 2 adds the cache]
- The world is split into **chunks** of **64 x 64 cells**. A chunk is the unit of storage, saving and threading. It is not the unit of simulation (section 5.1).
- A chunk is in one of four states:
  - **active:** some of its cells are inside the active region and are simulated;
  - **resident:** in memory, frozen;
  - **on disk:** saved because it differs from what the generator produces, not in memory;
  - **not generated:** never needed yet, or unchanged and dropped. It regenerates identically from the seed.
- **Resident cache:** resident chunks are kept up to a memory cap, set per machine (default 64 MB of chunk data on a 2-4 GB machine, about 2,000 chunks). Above the cap, the least recently active chunk is evicted: written to disk if it was changed, dropped if not. Disk writes go through the single writer thread (section 13) so panning never stalls. [rev 2: rev 1 never unloaded chunks, so long panning grew memory without limit.]
- Until saving exists (M4), changed chunks are never evicted; only unchanged chunks are dropped. Test scenes in M0-M3 are small enough for this. [rev 2.1]

### 4.3 Procedural generation [rev 2: rest guarantees, lakes, sea]
Generation is seeded and deterministic. The same seed and the same generator version always produce the same terrain.

Layers, generated in order:
1. **Base height:** a smooth noise field sets the surface. Large features (hills, valleys) come from low-frequency noise. Small features (bumps, rocks) come from high-frequency noise.
2. **Biome:** a second noise field chooses a biome per column. Biomes change slowly so the world has big regions.
3. **Strata:** below the surface, a layer of soil, then stone. Beaches and dunes use sand instead of soil. Depth of each layer varies with noise. [rev 2: rev 1 listed "dirt, then soil, then rock," but dirt and soil were one material and "rock" collided with cooled lava.]
4. **Water:** see the sea and lake rules below.
5. **Features:** caves, ponds, springs, boulders, trees, grass. Ore veins are v2.

**Rest guarantee.** A newly generated chunk must not move when it first becomes active. Otherwise every new chunk slides and drains as the player pans. The generator enforces:
- A powder surface (soil, sand) never has a slope steeper than that material's repose angle. Where the base height is steeper, the surface layer is stone.
- No unsupported solids: boulders sit on the surface; cave ceilings are stone and connected to the stone mass (section 8.7).
- Water is only placed where it is already level and contained. Soil and sand under generated water start saturated, and stone below them is impermeable, so a lake does not soak into its own bed. [rev 2.1]
- Caves are not carved within 8 cells of the surface below sea level, so the sea never drains into a cave.
- Generated cells start at the ambient temperature for their position and season (section 6.2), with the biome's default moisture.

This is tested (section 14.3): generated chunks are run for 600 steps and must stay nearly still.

**Sea.** A fixed sea level. Every open (non-solid) cell below sea level in a sea biome is water. The sea does not move in v1 except where the player disturbs it. The surface ripple is drawn by the renderer (art style: wave line), not simulated, so it costs nothing and cannot drain anything. Shore erosion by waves is v2. Tides are removed: a tide has to create and destroy water, which breaks the conserved-volume rule in section 5.7.

**Lakes.** Lakes use bounded feature placement instead of basin detection. Basin detection needs to know whether there is an outlet anywhere along the terrain, which a single chunk cannot know. Instead, lake centres come from a sparse noise. Each lake is a bowl at most 96 cells wide, carved and filled to its rim by the generator. Generating a chunk only looks at lake centres within 96 cells of it, so the cost is bounded.

**Generator version.** The generator has a version number, saved with every world (section 13).

Biomes at launch (name, look, what they do):

| Biome | Look | Behaviour | Priority |
|---|---|---|---|
| Plains | rolling grass, scattered trees | rain soaks soil, grass spreads, fires spread through dry grass, trees regrow | v1 |
| Hills and mountains | steep rock, sparse pines | rockfall, slow erosion on steep slopes, snow above the snow line | v1 (mountains) |
| Sea and beach | sand shore, open water | still sea with a rendered ripple | v1 |
| Desert | dunes, rock outcrops | wind moves sand, extreme heat, no rain | v2 |
| Volcano and lava | rock cone, lava pools | eruptions, lava flows, lava cools to basalt | v2 |
| Island | small land in sea | sea erodes the coast | v2 |
| Forest | dense trees | rain rots wood, fire spreads through trees | v2 |
| Shore erosion | — | waves move sand at the beach | v2 |

v1 ships plains, mountains, sea and beach. The rest follow once v1 is solid.

### 4.4 Spawn [my call]
The game starts on a plains-and-hill scene near the shore, so the first screen has water, grass, a hill and open ground. The seed is shown in the settings and can be typed in.

### 4.5 Surface heightmap [rev 2]
Several rules need to know whether a cell is open to the sky: rain landing, snow settling, drying, rot and weathering of exposed material, grass needing light, erosion, and ambient heat exchange. Rev 1 forbade a sky test because scanning a column every step is O(rows).

Fix: a **surface heightmap**, one entry per world column: the altitude of the highest **settled, non-plant, non-gas** cell in that column.
- **Settled** means a solid, or a powder or liquid that did not move last step. Falling raindrops, falling sand and particles never count, otherwise the heightmap would change every step during rain and the ground would flicker between sheltered and exposed. A powder or liquid joins the heightmap when it comes to rest. [rev 2.1]
- **Plants do not count.** Grass and leaves do not shelter what is below them: grass under a tree still gets light and the soil under grass is still the surface. Rain meeting a plant is handled in section 8.8. [rev 2.1]
- For a column that has never been changed, the surface is computed directly from the generator's height function, with no chunk loaded.
- When a cell settles or is removed, its column entry is updated: a settled cell above the surface raises it; removing the surface cell scans down to the next settled cell. That scan is rare and short, so the cost is O(changes), not O(cells).
- Heightmap entries are stored in strips of 64 columns, saved with the chunks that changed them.

Two kinds of exposure [rev 2.1: rev 2 had only the first, so a wall's side face or lava in a cave never exchanged heat]:
- **Sky-exposed** = the cell is the surface cell of its column. Sky-exposed cells receive rain, snow and sunlight.
- **Air-exposed** = the cell has at least one air neighbour (4-neighbour). Air-exposed cells exchange heat with the air (section 6.2), dry, and evaporate.
- **Sheltered** = below the surface of its column (under a roof, overhang or in a cave). A sheltered cell can still be air-exposed: the inside face of a cellar wall touches cellar air.

A cell under a roof is sheltered: it gets no rain or sun and dries slowly. That is the intended reading, and it is what makes a house protect what is inside it.

### 4.6 Terms [rev 2]
Rev 1 used "region" for three different things. From here on:
- **Active region:** the rectangle of cells being simulated: the view plus a 16-cell margin (section 5.1).
- **Weather column:** a 512-cell-wide (8-chunk) vertical strip of the world with its own weather values (section 9.1).
- **Ambient temperature** is a function of position and time (section 6.2), not a region.

---

## 5. Performance architecture

This section decides whether the game runs on an i3. Nothing else matters if it does not.

### 5.1 Viewport-only simulation [the user's requirement]
- Only cells inside the **active region** are simulated.
- The active region is the visible view plus a margin of **16 cells** on each side. The margin hides the seam where simulation starts and stops.
- **The clip is per cell, not per chunk.** [rev 2] Chunks are 64 x 64, so simulating every chunk that overlaps the active region would simulate up to 63 extra cells on each side, which brings back the cost the 16-cell margin was meant to remove. Each active chunk iterates only the part of its cells that lies inside the active rectangle.
- The margin was one chunk (64 cells) and that was too expensive: at 720p it turns 57,600 visible cells into 138,000 active cells, 2.4x the visible count, mostly spent on cells nobody sees. 16 cells costs 30% at 720p. Tune 16 against fire-front visibility in M2 (section 5.2).
- Cells outside the active region are **frozen**. Their state does not change. When they come back into view, they resume from where they stopped.

### 5.2 Frozen off-screen [decided]
The user chose frozen for v1. A house left alone for a season is exactly as it was left. Aging, weathering and fire spread do not continue while off-screen.

Consequence to design around: a fire near the edge of the view stops at the margin. It resumes when the camera moves back. Players will see this, so the margin must be large enough that fire fronts are not stopped in view.

Frozen also applies to slow processes: random ticks (section 7.4) only pick cells inside the active region, so off-screen chunks never age and need no timestamp reset when they return to view.

### 5.3 Ticks: 20 TPS base rate and fast-forward [rev 2.4: frame budget, overdue discard and exact remainder]
Rev 1 ran 60 physics steps per second, leaving no headroom for fast-forward on the i3. The simulation uses a **20 TPS tick model** scheduled by elapsed time:

- **Base tick rate and frame simulation budget:** **20 ticks per second (50 ms period)** at 1x speed, while rendering runs at 60 FPS (16.67 ms frame period). Because ticks run synchronously on the main loop between frames, **both a single tick and the total simulation time across all ticks in a frame must stay inside the 60 FPS budget**:
  - **Single-tick target:** $\le 10\text{ ms}$ on the i3 (leaving headroom for rendering and OS overhead).
  - **Per-frame total simulation budget ($B_{\text{frame}}$):** $\le 12\text{ ms}$ per frame. A frame may run multiple ticks only while their combined elapsed time stays within $B_{\text{frame}}$ (so two 10 ms ticks never run in the same frame; if one tick takes 8-10 ms, that frame runs $k = 1$ tick).
- **Elapsed-time scheduling, overdue discard and pause/speed reset:**
  - While unpaused, each frame measures wall-clock $\Delta t$ (clamped to at most 250 ms so an OS stall or window drag never builds a backlog) and adds $\Delta t$ to `step_time_accum` and $\Delta t \times 20 \times \text{speed}$ to `world_tick_accum`.
  - While **paused**, $\Delta t$ is not added to either accumulator. When toggling pause or changing the speed setting, both `step_time_accum` and `world_tick_accum` are reset to 0 so no banked time carries across speed changes.
  - Let $\tau = 50\text{ ms} / \text{speed}$ be the target tick period, and $\text{due} = \lfloor\text{step\_time\_accum} / \tau\rfloor$. If $\text{due} = 0$, no simulation tick runs this frame.
  - If $\text{due} \ge 1$, the planned tick count is $k_{\text{plan}} = \max(1, \min(\text{due}, N, \lfloor B_{\text{frame}} / \bar{t}_{\text{tick}}\rfloor))$, where $N$ is the per-frame tick cap (expected: $N = 2$ on the i3, $N = 8$-$16$ on a Mac) and $\bar{t}_{\text{tick}}$ is the moving-average tick duration.
  - **Overdue physics ticks are discarded, never banked:** `step_time_accum` is set to $\text{step\_time\_accum} \bmod \tau$ (keeping only the sub-period fractional remainder $< \tau$). All capped ticks $(\text{due} - k_{\text{plan}})$ are folded into the tick multipliers $M_i$ below rather than left in `step_time_accum`, so switching from `64x` back to `1x` never plays out a backlog of excess physics ticks.
- **Integer tick counters:** time is measured in integer ticks, with no floating-point simulation clock:
  - `step_tick: u64` counts simulation ticks actually executed (+1 per tick);
  - `world_tick: u64` counts in-game calendar ticks (24,000 ticks = 1 day, section 6.1).
- **Exact multiplier and remainder mechanism:**
  - When $k_{\text{plan}} \ge 1$ ticks are scheduled for the frame, let $W = \max(k_{\text{plan}}, \lfloor\text{world\_tick\_accum}\rfloor)$ be the integer world ticks to distribute.
  - Before each tick $i \in \{1, \dots, k_{\text{plan}}\}$ (with $k_{\text{rem}} = k_{\text{plan}} - i + 1$ ticks remaining and $W_{\text{rem}}$ world ticks remaining), tick $i$ is assigned the integer multiplier $M_i = \lceil W_{\text{rem}} / k_{\text{rem}} \rceil \ge 1$. When tick $i$ completes, $W_{\text{rem}}$ and `world_tick_accum` are decremented by $M_i$.
  - If elapsed simulation time in the frame exceeds $B_{\text{frame}}$ after tick $i < k_{\text{plan}}$, the loop stops early after tick $i$: any unspent $W_{\text{rem}}$ remains in `world_tick_accum` for the next frame, so calendar time is still conserved without breaking 60 FPS.
- When $M_i > 1$, fast reactions (falling, flowing, fire) run at the machine's capped tick rate while seasons, day-night and slow processes advance by $M_i$ world ticks on that tick (section 7.4), and the time bar shows "physics limited". Nothing freezes.
- Rendering runs at the display rate. The cell grid is drawn stepped (latest tick state); particles (section 8.5) are interpolated between ticks so blast debris stays smooth at 60 FPS.

### 5.4 Budget [my call, to be measured]
- **The budget is a number of active cells per machine**, measured in M0, not a resolution. A cell is a fixed 4 physical pixels, so the window decides the cell count; when the window holds more than the budget, the view is capped (section 3.3). Simulation runs on the active region, so counts below include the 16-cell margin.

| Machine | Window | Cells visible | Active cells (with margin) | Fits budget? |
|---|---|---|---|---|
| i3 (floor) | 1280 x 720 | 57,600 | 74,624 | yes: this defines the floor budget |
| i3 laptop | 1366 x 768 | 65,472 | 83,552 | measure; capped view if not |
| i3 | 1920 x 1080 fullscreen | 129,600 | 154,624 | no: view capped to about 74,624 active |
| Mac | 1920 x 1080 | 129,600 | 154,624 | expected yes |
| Mac | 2560 x 1440 | 230,400 | 263,424 | expected yes |
| MacBook Pro 14" | 3024 x 1964 fullscreen | 371,196 | 412,124 | measure |
| MacBook Pro 16" | 3456 x 2234 fullscreen | 482,112 | 528,640 | measure |

- Target: **60 FPS on the i3 at 1280 x 720** (74,624 active cells). On the Mac, 60 FPS at its native fullscreen size, capped if the measurement says so.
- 154,624 active cells (1080p) is the reference load for benchmarks: roughly double the i3's floor.
- **Cell layout: exactly 8 bytes**, a flat array per chunk. [rev 2: rev 1 listed more fields than fit.]

| Field | Size | Meaning |
|---|---|---|
| material | u8 | material id |
| temperature | i16 | degrees C, -32,768 to 32,767 (lava needs more than a u8) |
| moisture | u8 | 0 (dry) to 100 (soaked) |
| timer | u8 | short-lived counter: burn time, gas lifetime, fuse |
| progress | u8 | slow-process progress, meaning set by material: rot, rust, growth stage, leaf state |
| flags | u8 | update parity bit, slow-tick parity bit, rooted/attached bit, falling-group bit, settled bit, burning bit, 2 spare |
| variant | u8 | colour variation and visual sub-type (flower, pine needle, wet tint) |

- "Age" from rev 1 is replaced by `progress`. Slow processes advance `progress` on random ticks (section 7.4), so no per-cell or per-chunk timestamp is needed.
- **Air stores no temperature.** An air cell's temperature is the ambient function (section 6.2). Its temperature field is unused. This removes most of the heat work, because most visible cells are air.
- **Dirty rectangles:** each chunk tracks the rectangle of cells that moved or changed temperature last tick. A chunk at rest, all within 1 C of ambient, skips its movement and heat passes. Most of a calm landscape is at rest, so the real per-tick cost is far below "all active cells."
- **Particles** (section 8.5) have their own budget: 4,096 on the i3, 32,768 on the Mac.
- Multi-threading: chunks are split across CPU cores with the four-phase scheme in section 5.8. A first-generation i3 has 2 cores and 4 threads.
- **Budget watchdog** [rev 2.4]: rev 1 lowered the number of active cells, which would freeze part of the visible screen (an anti-goal). Instead, when simulation time exceeds $B_{\text{frame}}$ (12 ms):
  1. during fast-forward, the $B_{\text{frame}}$ guard in section 5.3 reduces the number of ticks run per frame $k$ toward 1 (down to 20 TPS), increasing the tick multiplier $M$ and showing "physics limited";
  2. if a single tick at 20 TPS still exceeds the frame budget, run fewer than 20 ticks per second. The whole world slows uniformly, and the time bar shows "slowed".
  No part of the screen is ever frozen while the rest moves.

### 5.5 Measurement plan
Before writing any feature beyond sand and water, measure:
- Frame time for 74,624 sand cells on the i3 (1280 x 720, with margin), single-threaded and with the four-phase scheme on 2 and 4 threads.
- The same scene at 154,624 cells and at native fullscreen on the Mac.
- Frame time with 50% of cells water (water is heavier to update).
- Memory per resident chunk, and total memory after panning for 10 minutes.
- **Renderer check on the i3's GPU** [rev 2]: confirm that macroquad 0.4.16 opens a window and draws the grid on the test machine's integrated graphics (Intel HD Graphics, OpenGL 2.1 class). This has not been verified. If it fails, the floor machine or the graphics library changes before anything else.

If the numbers do not hit the target, the design changes before features are added, not after.

### 5.6 Platforms [my call]
- **Language:** Rust.
- **Graphics:** macroquad 0.4.16, as decided earlier. It runs on Mac, Windows and Linux, and it is light. Subject to the GPU check in section 5.5.
- **Simulation:** CPU, multi-threaded. Not GPU-only, because an i3 has no usable GPU compute. A GPU path can come later for high-end machines, but it is not required.
- **Save format:** compressed region files (section 13).

### 5.7 The region boundary [decided 2026-11-06, rev 2 changes the water cycle]
The active region is 16 cells wider than the view on each side, so anything that crosses the margin has already left the screen. The rule depends on whether the material is conserved:

- **Liquids, powders and particles stop at the margin.** The boundary acts as a solid wall for anything with mass. Volume is conserved: a lake half off-screen does not drain, wind does not blow sand permanently out of the world, and a dam holds. When the camera pans, the material resumes and spreads into the newly active cells.
- **Gases are absorbed at the margin.** Smoke, steam and ash are not conserved. They are visual and short-lived and they already fade or condense on their own, so a gas that crosses the margin is simply removed. It is at least 16 cells off-screen when that happens, and a gas rising 2 cells per tick covers that margin in 0.4 s (8 ticks at 20 TPS), so the removal is never in view.
- **The water cycle runs through the weather, not the view.** [rev 2] Rev 1 turned steam into cloud at the top of the view, so the cloud height moved with the camera. Now:
  - steam carries its own temperature and loses heat to the air every step (section 8.2). When it falls below its condensation point it becomes a water droplet. Steam's cooling rate is tuned so this happens a short, roughly fixed distance above where the steam formed (target: 40-80 cells), at any altitude and wherever the camera is [rev 2.1: rev 2 tied condensation to an absolute altitude, so it depended on terrain height and might lie outside the view];
  - steam absorbed at the margin, and water that evaporates (section 8.8), adds to the humidity of its weather column;
  - rain is drawn from the weather column's humidity (section 9.2).
  Nothing has to survive off-screen for the cycle to work, and the camera does not change where condensation happens.
- **Fire does not cross the margin.** It behaves as a gas. This is the limitation described in section 5.2 and it is tested in M2.
- **No queue and no reflection.** A queue grows without bound during a long fire, and a reflection shows the player a wall at the edge of the screen. Absorbing off-screen is the only option that costs nothing and leaves no artefact.

### 5.8 Update order, threads and determinism [rev 2]
- **Four-phase chunk update.** Active chunks are split into four sets by (chunk x mod 2, chunk y mod 2). Each phase updates one set in parallel. No two chunks updated at the same time are neighbours, so a cell may move into a neighbouring chunk without a data race. A cell may move at most 31 cells in one step, and reads at most one cell beyond its destination, so every read and write stays within 32 cells (half a chunk) of its own chunk. Two chunks in the same set therefore never touch the same cells of a shared neighbour. [rev 2.1: rev 2 said 32, and the destination check made it 33.]
- **Serial work after the phases:** falling groups (section 8.7), particles (section 8.5) and support flood fills can span many chunks, so they run on one thread after the four phases, in a fixed order.
- **Update parity bit:** a cell that moved this step is marked, so it is not moved twice when a later phase or row reaches it.
- **Scan order:** bottom row to top, and the horizontal direction alternates every step, so powders and liquids show no left/right bias.
- **Random numbers:** every random choice uses a hash of (world seed, `step_tick`, cell position). Results are identical whatever the thread count or timing, which the tests rely on (section 14.1).
- **What determinism covers** [rev 2.2]: the world is a pure function of (starting state, inputs, and the integer tick multiplier $M$ per tick). Random ticks run once per simulation tick using `step_tick`, so frame timing never enters. In the live game the watchdog may adjust $M$ when a machine is overloaded; headless tests fix $M$ and are 100% reproducible.
- **Threads in M0:** the four-phase loop is written from M0 as a sequential loop. Turning on threads is a change to how phases are dispatched, not a rewrite. Threads are switched on in the first milestone whose measurement needs them (section 15).

---

## 6. Time, seasons and day-night

### 6.1 Clock [my call, rev 2.2: integer ticks]
- One in-game **day** = **24,000 ticks** (**20 real minutes** at 20 TPS, 1x speed).
- One in-game **season** = **3 in-game days** = **72,000 ticks**.
- One in-game **year** = 4 seasons = **12 in-game days** = **288,000 ticks** (4 real hours at 1x, or 3.75 minutes at 64x).
- Time is stored as two integers saved with the world: `world_tick: u64` (in-game clock) and `step_tick: u64` (ticks executed, section 5.3).

### 6.2 Day, night and ambient temperature
- The sun moves across the sky with the time of day.
- Light level and sky colour change with the time, following the art style: sunrise and sunset use the warm dusk palette; night uses the navy palette; midday uses the blue and pale haze.
- Night has stars and a moon. The moon's phase follows the day count.
- Temperature drops at night and rises at midday. This affects water freezing, snow melting and steam.

**Ambient temperature** [rev 2: rev 1 applied it to the top band of the view and relied on conduction through air to carry it down. Conduction time grows with the square of distance, so it would barely reach the ground; and it made temperature depend on where the camera was.]

- Ambient is a **function**, not a field: `ambient(x, altitude, time) = base(season, time of day) + weather(weather column) - lapse x (altitude - sea level)`. It costs a few arithmetic operations to evaluate and stores nothing.
- The **lapse rate** makes the air colder with height. The **snow line** is simply the altitude where ambient falls below 0 C; it rises in summer and falls in winter on its own.
- **Air cells read ambient directly** (section 5.4). Air above its column's surface reads open-air ambient. Air below the surface (caves, rooms) reads a **sheltered ambient**: the season's mean temperature, with no day-night swing.
- **Air-exposed cells** (section 4.5) exchange heat each step with the air they touch, at their material's rate. [rev 2.1: rev 2 limited this to the top cell of each column, so lava in a cave or the side of a hot wall never cooled.] A pond freezes from the surface down. Exposed snow melts on a warm afternoon. A buried cellar stays cool in summer and above freezing in winter. That is the intended reading.
- Cells with no air neighbour exchange heat with their neighbours only (section 8.2).

### 6.3 Seasons
- **Spring:** rain is more frequent, grass grows faster, leaves regrow on trees.
- **Summer:** dry, fires spread faster, rain is rare, evaporation is fastest.
- **Autumn:** leaves detach and fall (section 7.4), grass dies back.
- **Winter:** cold, snow falls above the snow line, ice forms on still water, snow melts when warm.

Season length and effects are tuned by playtest. The player should see a change within minutes: at 4x a day is 5 minutes, so a 3-day season ends after 15 minutes, and at 16x the same season takes under 4 minutes.

---

## 7. Materials

Every material has a simple property sheet. Behaviour comes from a short list of rules, not from one rule per material pair.

### 7.1 Property sheet (fields every material has)
- **Kind:** gas, liquid, powder (sand-like), solid, or plant.
- **Density:** decides what sinks and floats.
- **Conductivity:** how fast the material exchanges heat with neighbours and, when exposed, with ambient.
- **Ignition temperature:** if the cell reaches this, it may burn. Blank if not flammable.
- **Porosity:** the most moisture the material can absorb. 0 for stone and metal.
- **Hardness:** how much erosion or blast it takes.
- **Repose angle:** for powders, the steepest stable slope. Wet sand and soil have a steeper angle (they stick).
- **Paint temperature:** the temperature a painted cell starts at (section 10). Blank means "ambient at that position."

### 7.2 Launch materials (v1) [rev 2.2: completed with base hex colours]

Base daytime/neutral colours below match the muted palette in `2026-11-06-art-style.md` (and shift with time-of-day tinting and wet/dry state):

| Material | Kind | Density | Base colour | Notes |
|---|---|---|---|---|
| Air | gas | 0 | (sky / cave bg) | Empty cell. Stores no temperature; reads ambient. |
| Water | liquid | 1.0 | `#2b5878` (rim `#5f80b0`) | Flows, finds its level, boils at 100 C, freezes at 0 C, evaporates slowly when exposed. |
| Steam | gas | 0.6 | `#cfd8dc` (65% alpha) | Rises, cools, condenses to water. |
| Sand | powder | 1.6 | `#e2b679` (wet `#b58c54`) | Falls, makes piles, wet sand is darker and sticks. |
| Soil | powder | 1.4 | `#4a3b32` (wet `#2e241e`) | Like sand, holds more moisture, grows grass and trees. (Rev 1's "dirt / soil" is one material.) |
| Grass | plant | 0.3 | `#3b5e3c` (dry `#9e8c4e`) | One cell on top of soil. Grows in light and moisture. Dries and burns. Flowers are a grass variant (`#d88c7a`). |
| Stone | solid | 2.6 | `#4a5568` (rim `#6d7d94`) | Static while supported (section 8.7). Erodes very slowly. |
| Wood | solid | 0.6 | `#6e4f3a` (wet `#4d3627`) | Burns, rots when wet for long. Tree trunks are wood with the rooted flag. |
| Rotten wood | solid | 0.5 | `#3d352e` | Made by rot. Weak (low hardness). Crumbles to soil over days. |
| Leaves | plant | 0.2 | `#2f4f38` (autumn `#b86b35`) | Attached to trees, detach in autumn and drift down. Burns quickly. Pine needles (`#1f362a`) are an evergreen variant that never detach. |
| Metal | solid | 7.8 | `#7c8794` | Conducts heat well. Rusts when wet. |
| Rust | powder | 2.0 | `#8c4a2f` | Made by rust. Flakes off metal, washes away in rain. |
| Lava | liquid | 2.5 | `#e86a2c` (core `#f5a642`) | Very hot. Cools to basalt. |
| Basalt | solid | 2.7 | `#282d38` | Created by cooling lava. (Rev 1 called this "rock," which clashed with the strata.) |
| Snow | powder | 0.3 | `#e6ebf4` | Falls in cold. Melts above 0 C. |
| Ice | solid | 0.9 | `#8ab4d4` | Forms from water below 0 C. Melts. |
| Fire | gas-like | — | `#f28c38` (tip `#fbe0a8`) | Flames: short-lived cells emitted into the air above and beside a burning cell. A burning cell itself keeps its material and sets its burning flag (section 8.3). |
| Smoke | gas | 0.3 | `#434952` (55% alpha) | Rises from fire, fades. |
| Ash | powder | 0.8 | `#6e6a66` | Left after burning. Becomes soil over days when on soil. |
| Gunpowder | powder | 1.8 | `#33363b` | Explodes when ignited. |
| Oil | liquid | 0.8 | `#2b2623` | Floats on water. Burns. |

Clouds are not a material. They are drawn in the sky layer from the weather column's humidity and cloud cover (art style: sky). Trees, pines and flowers are made of the materials above, not separate materials.

### 7.3 Long-term materials (v2 and later)
Clay, mud, moss, reeds, mushrooms, coal, ore, glass, brick (player-built), wood beams (player-built), wet rock, and salt.

### 7.4 Slow processes: random ticks [rev 2.4: distinct per-tick picks, slow-parity guard and bounded approximation]
These are the reasons the world looks old. Instead of scanning every cell or maintaining per-stripe timestamps, they use **Minecraft-style random ticks**:

**How they run at $M = 1$ (normal tick):**
- Every tick with $M = 1$, each active chunk picks **$K = 16$ distinct cells** (`randomTickSpeed = 16`) using a seeded permutation step `(start + i * stride) & 4095` (with odd `stride` derived from `(world_seed, step_tick, chunk_pos)` so all $P \le 256$ picks within a chunk in one tick are guaranteed distinct without replacement). If a picked cell lies outside the active region (in a boundary chunk), it is skipped.
- At $K = 16$ and 20 TPS, each 64 x 64 chunk (4,096 cells) gets 320 picks per second, so on average every cell is visited once every 12.8 seconds (256 world ticks, or **93.75 visits per cell per in-game day**).

**Enforcing at most one slow transition per cell per tick [rev 2.4]:**
- Because the permutation stride has no replacement, the same cell is never picked twice in the same tick.
- In addition, every cell has a **slow-tick parity bit** in `flags` (section 5.4). Before a picked cell runs its slow-process rule, if its slow-tick parity bit already matches `step_tick & 1`, the cell is skipped; otherwise its slow-tick parity bit is set to `step_tick & 1`. Whenever a slow rule creates or modifies another cell (grass spreading to neighbour soil, a tree adding a trunk/leaf cell, erosion moving a cell downhill, wood turning to rotten wood), it also sets the target cell's slow-tick parity bit to `step_tick & 1`.
- This guarantees that in a single simulation tick, no cell can undergo two slow transitions (such as wood $\to$ rotten wood $\to$ soil) and no newly created/modified neighbour can be updated again in the same tick.

**How they run at $M > 1$ (fast-forward above the per-frame tick cap):**
- If a tick with multiplier $M > 1$ still picked only $K = 16$ cells, expected visits per in-game day would drop by $1/M$ (down to ~5.86 visits/day at $M = 16$), increasing variance and skipping intermediate states.
- Instead, a tick with multiplier $M$ runs **$P = \min(K \times M, K_{\max})$ distinct random picks per active chunk**, with $K_{\max} = 256$ picks/chunk/tick, and sub-step factor $m = (K \times M) / P$:
  - **For $M \le 16$ (covers up to 64x speed at 80 TPS):** $P = 16 \times M \le 256$ and $m = 1$. Every pick is a standard unit step, and every cell still receives **93.75 expected visits per in-game day** with no step-size distortion. At 720p, the $352 \times 212$ active region intersects 24 to 35 chunks depending on camera alignment ($\lceil 352/64 + 1\rceil \times \lceil 212/64 + 1\rceil = 7 \times 5 = 35$ maximum): at $P = 256$ that is at most $35 \times 256 = 8{,}960$ index checks per tick, of which ~4,664 land inside the 74,624 active cells (~6.2% of active cells).
  - **Fallback when $M > 16$ ($m > 1$):** when a slow machine is forced above $M = 16$, $P$ is capped at 256 picks/chunk/tick and each pick applies a bounded sub-multiplier $m = (K \times M) / 256 > 1$:
    1. **Probabilities never use $m \times p$:** a rule with base per-pick probability $p \in [0, 1]$ fires with probability $p_{\text{eff}} = 1 - (1 - p)^m \in [0, 1]$ (the continuous survival-probability extension of $m$ independent trials, reaching $1$ only when $p = 1$).
    2. **No stage skipping:** a progress counter advances by $\min(\lfloor m \times \text{rate}\rfloor, \text{stage\_cap})$, clamping at the next state transition. Together with the slow-tick parity bit above, a single simulation tick can turn wood into rotten wood, never wood straight into soil; a tree adds at most one template cell per tick; seepage and drying clamp at neighbour/ambient equilibrium.
    3. **Accepted approximation at $M > 16$:** spatial chains (such as grass spreading across multiple cells or multi-cell tree growth) and multi-stage decay chains can advance at most one step per simulation tick per cell. This is verified at 1x, 16x and 64x (section 14.1).
- Chunks store no slow-process timestamps at all.

**Decay:**
- **Rot (wood):** wood with moisture above a threshold gains rot progress. At full progress it becomes **rotten wood**, which crumbles to soil over more days. Dry wood does not rot.
- **Rust (metal):** metal with moisture above a threshold gains rust progress. At full progress the cell becomes **rust** (powder), which falls and washes away. Exposed surfaces rust first, so a metal sheet loses cells from the outside in. That is what "thinner" means on a cell grid.
- **Erosion (rain):** when a sky-exposed loose cell (soil, sand, ash, rust) on a slope is picked during rain (or while soaked from rain), it may move one cell downhill. Repeated over days, this cuts channels. Stone erodes at a small fraction of that rate. Rates are tuned so a hill visibly gains channels in one in-game day at 16x but keeps its shape for many in-game years.
- **Drying:** see section 8.8.
- **Weathering:** air-exposed stone and wood change colour slightly over seasons, through `variant` (art-style detail, no gameplay effect).

**Renewal** [rev 2: rev 1 had no way for the world to recover, so an endless sandbox would end as flat ash]:
- **Grass growth:** grass spreads to sky-exposed soil next to grass when light and moisture are both above thresholds. Grass that is buried (a settled powder, liquid or solid directly on top of it) for about one in-game day dies back to soil. [rev 2.1]
- **Soil formation:** ash and leaves lying on soil become soil over days. A burned hillside becomes fertile again.
- **Trees:** a sky-exposed grass cell with high moisture and space above has a small chance per day to sprout a sapling. A sapling grows into a small tree over several days, adding one trunk or leaf cell per growth tick from a simple template. Trunks are wood with the rooted flag.
- **Leaf cycle:** in autumn, attached leaves detach and drift down (section 8.1) and become litter. In spring, living trees regrow leaves at their branch tips.

Each slow process has a visible result within the first in-game day so the player notices it. The final rate is tuned by playtest.

---

## 8. Physics rules

Physics is simple. Each rule is a local test on a cell and its neighbours, applied each step. The rules combine into the behaviours the player sees. The two exceptions, support and particles, are run only when something changes.

### 8.1 Movement
- **Gravity:** powders and liquids fall up to **3 cells per tick** (60 cells/s at 20 TPS, matching 60 Hz visual fall speed), stopping at the first cell along the path that is not lower density. [rev 2.2: 1 cell/tick at 20 TPS took 13.5 s to cross a 1080p screen and broke the section 8.4 time limits.]
- **Powder piles:** if the cell below is blocked, powders slide diagonally down (up to 2 cells per tick), but only while the slope is steeper than their repose angle. This makes piles with natural slopes.
- **Liquids:** water spreads sideways to find its level, so pools flatten. Sideways spread is at most 31 cells per tick (section 5.8).
- **Gases:** rise up to **2 cells per tick** (40 cells/s) if lighter than air, and drift with wind.
- **Solids:** do not move while supported (section 8.7).
- **Plants** [rev 2: rev 1 gave plants no movement rule]: a plant with the rooted/attached flag does not move, and other material treats it as solid: powders rest on it, and liquids are absorbed by it (section 8.8). Grass is rooted while soil is directly below it. Leaves are attached while they touch a living trunk or another attached leaf. A plant that loses its root or attachment becomes unattached and moves as a light powder: it falls slowly, drifts with the wind, and settles as litter.

### 8.2 Heat
- Every non-air cell holds a temperature. Air reads ambient (section 6.2).
- Each step, each cell exchanges heat with its non-air neighbours at a rate set by the two materials' conductivity. Metal exchanges fast. Water exchanges moderately.
- Air-exposed cells also exchange heat with the air they touch (section 6.2).
- Heat does not conduct across air gaps. It crosses air only by being carried by flame, smoke and steam cells, which rise. [rev 2: replaces "air moves heat by rising," since air has no stored temperature.]
- Lava holds a high temperature. A burning cell and its flames hold a high temperature while it burns. Ice and snow hold low temperatures.
- Water above 100 C becomes steam. Water below 0 C becomes ice.
- Steam loses heat to the air faster than other materials, so it condenses a short distance above where it formed (section 5.7). Steam below its condensation point becomes water.

### 8.3 Fire [rev 2.1: burning is a flag, not a material change]
Rev 2 turned a burning cell into a fire cell, which is gas-like. A burning wall would lose its support the instant it caught, and a burning house would collapse in one frame instead of burning down.
- A flammable cell (wood, rotten wood, grass, leaves, oil, gunpowder) catches fire when its temperature reaches its ignition point.
- **A burning cell keeps its material** and sets its burning flag. Its `timer` counts down its remaining fuel. It stays in place, stays part of its support group, and holds a high temperature. When the timer runs out, it becomes ash (powder) or air. That is when the structure loses the cell, so a burning house sags and falls apart as its beams burn through.
- **Flames** are fire cells: short-lived, gas-like cells that a burning cell emits into air next to it, mostly upward. They are what the player reads as fire. They carry heat, rise, drift with the wind, and fade after a few steps.
- **Spread:** a burning cell and its flames heat adjacent flammable cells each step. Dry material heats faster than wet material. With wind, flames lean downwind and heat downwind neighbours more, so fire spreads faster in the wind's direction.
- **Moisture:** water or rain on a cell raises its moisture and lowers its effective fuel value. A cell that is soaked does not burn, and soaking a burning cell clears its burning flag. Heat dries a cell before it can ignite it, which produces steam from wet wood.
- **Out:** a burning cell whose fuel runs out stops burning. Flames with no burning cell next to them fade. Flames touching water are removed.

### 8.4 Required reactions (must work, verified by test and by playing)

These are the behaviours the design depends on. Each one has a test and a visible time limit. Times are in real seconds at 1x unless stated.

| Cause | Effect | Max time to be visible |
|---|---|---|
| Lava touches water | Water boils to steam immediately, lava cools to basalt at the contact. | 1 second |
| Hot sand (above 100 C) touches water | Water boils, slower than lava. | 3 seconds |
| Fire touches dry wood | Wood catches fire and fire spreads along the wood. | 3 seconds |
| Fire touches wet wood | Wood does not catch. Steam forms if water is hot enough. | 3 seconds |
| Rain falls on fire | Fire goes out. | 2 seconds |
| Gunpowder touches fire | Explosion. | 1 second |
| Explosion | Throws nearby material outward as particles, starts fires, leaves a crater. | 1 second |
| Water poured on high ground | Flows downhill and pools in low ground. | 5 seconds |
| Sand dug from under a free stone slab | The slab falls into the hole as one piece (section 8.7). | 3 seconds |
| Floating structure (no cell resting on anything) | Falls as one piece. | 1 second |
| Steam rises and cools | Condenses into droplets that fall back as water (section 5.7). | 15 seconds |
| Water exposed to air below 0 C | Freezes from the surface down. Ice melts above 0 C. | 10 seconds |
| Snow in warm air | Melts to water. | 10 seconds |
| Wet soil in sun | Visibly dries (lighter colour). | 1 in-game day at 16x |
| Wood left wet for days | Rots, then crumbles to soil. | 1 in-game day at 16x |
| Metal left wet for days | Rusts; rust flakes off. | 1 in-game day at 16x |
| Rain on bare soil on a slope | Soil washes downhill, channel forms. | 1 in-game day at 16x |
| Burned ground left alone | Grass returns. | 1 in-game season at 64x |

If a required reaction fails a visibility test, the rule set changes, not the reaction list.

### 8.5 Explosions [my call, rev 2 adds particles]
- Gunpowder and other explosive materials release energy when ignited.
- Energy spreads as a blast radius. Inside the radius, cells are destroyed or thrown, depending on hardness.
- **Thrown cells become particles** [rev 2: cells have no velocity, so rev 1 had no way to "throw" them]. A particle is removed from the grid and kept in a separate list with a position and velocity (floats). It flies under gravity and is drawn as a single cell. When it hits a non-gas cell, it re-enters the grid in the nearest free cell.
- Particles are capped (section 5.4). Above the cap, the extra cells in the blast are destroyed instead of thrown. Particles that reach the margin stop there and re-enter the grid, like any material with mass (section 5.7).
- Blast creates heat, which can start fires in nearby flammable material.
- Blast creates a short-lived shockwave effect (a single flash frame and a sound) for readability.

### 8.6 Wind and atmosphere
- Wind is part of the weather field (section 9.1): a direction and strength per weather column, changing slowly over time.
- Gases and light powders (ash, smoke, snow, unattached leaves) drift with the wind.
- Wind strength rises during storms. Strong wind moves leaves and light powders, and spreads fire faster in its direction.
- Pressure is not simulated separately. Gases move to equalise, which is enough for smoke and steam to behave.

### 8.7 Support [rev 2]
Rev 1 said a solid falls when it has "no solid below or beside it." That is a local test, so any row of two or more solid cells holds itself up in mid-air: a floating roof would never fall.

The rule now works on **connected groups**:
- Rigid solids are stone, wood, rotten wood, metal, ice and basalt. Two rigid cells next to each other (4-neighbour) belong to the same group.
- A group is **supported** if at least one of its cells rests on a non-gas, non-liquid cell directly below it that is not part of the group: powder, a different supported group, or the ground. The bedrock floor of the world (section 4.1) is always supported.
- **Floating** [rev 2.1: without this, ice floes and wooden rafts would sink]: a group whose average density is lower than the liquid under it is held up by that liquid. It rests at the waterline. A group denser than the liquid sinks through it, swapping places with the liquid cells it moves into.
- **The check runs only on change.** When a rigid cell is removed, or the cell below a rigid cell changes, a flood fill starts from the affected neighbours to find their group and test it. Most steps have no such change, so the cost is O(changes).
- **Bounds:** the fill stops at 4,096 cells or at the active region's edge. Either case counts as supported. Terrain masses are huge and anchored; a group that reaches off-screen is assumed to be held up there (it is frozen there anyway).
- **An unsupported group falls as one rigid piece**, up to **3 cells per tick** (60 cells/s), keeping its shape, until any cell of it would land on something along that path. Then it stops and is checked again. A falling group is held in a small list and moved as a unit, in the serial pass after the parallel phases (section 5.8). A falling group that reaches the margin stops there, like any material with mass (section 5.7).
- **Impact:** groups made of weak materials (rotten wood, ice) that fall more than 8 cells break apart on landing: their cells become loose powder and fragments. Strong groups land intact.
- No stress model: an arbitrarily long cantilever that is attached to supported ground holds. A maximum span can be added later if bridges feel wrong in playtest.

This produces the intended lesson in section 10: a roof with no wall under it falls; a roof on walls stands; digging out the sand under a free slab drops it.

### 8.8 Water balance: absorption, runoff, evaporation, drying [rev 2]
Rev 1 had no way for water to leave the world except boiling, and moisture never went down. Rain would flood the world over time, and dry grass would never be dry again.

- **Absorption:** a water cell (rain or poured) that lands on a porous cell (soil, sand, ash, wood) whose moisture is below its porosity is absorbed: the water cell is removed and the cell's moisture rises.
- **Plants** [rev 2.1]: a water cell that meets a grass or leaf cell is absorbed by the plant. A saturated plant passes moisture to the cell below it: grass to its soil, a saturated leaf by releasing a water drop into the air below. Rain on a tree drips through the canopy; rain on a lawn soaks the soil.
- **Runoff:** a saturated cell absorbs nothing, so water stays as a water cell, flows downhill and pools. Light rain soaks in; heavy or long rain makes puddles and streams.
- **Seepage:** moisture moves slowly from wetter cells to drier porous cells below them (slow process), so soil dries at the surface first and stays damp deeper down. Seepage stops at non-porous cells (stone, metal, basalt, ice).
- **Drying:** air-exposed cells lose moisture over time (slow process). The rate rises with temperature and wind, is highest for sky-exposed cells in daytime sun, and is near zero for cells with no air neighbour. Grass under a roof stays damp; grass on an open hillside in summer dries and becomes fire fuel.
- **Evaporation:** an air-exposed water cell with air above it has a small chance per in-game hour to evaporate, faster when warm, sunny and windy. Puddles disappear in a day or two; a lake loses a barely visible amount.
- **Humidity pool:** evaporated water, absorbed steam and the rain-cloud tool add to the weather column's humidity. Rain removes from it (section 9.2). This closes the cycle. It is approximate, not exact accounting: the weather column's baseline humidity comes from the weather noise (section 9.1), and player-caused changes decay back toward it over about one in-game day.

---

## 9. Weather and events

### 9.1 Weather field [my call, rev 2]
- The world is divided into **weather columns** 512 cells wide. Each has humidity, cloud cover, rain rate, wind and a temperature offset.
- Each value is a **function of (weather column, world time)**: seeded noise that drifts sideways with the wind over time, so weather fronts cross the screen. On top of that, each column stores a small **delta** from player and simulation effects (evaporation, rain-cloud tool), which decays toward zero over about one in-game day.
- Because the baseline is a function, weather needs no off-screen simulation and does not conflict with frozen chunks (section 5.2): weather is evaluated where it is needed. Only the deltas are state, and they are saved.
- **No edges at column borders** [rev 2.1]: the weather at a world column x is interpolated linearly between the two nearest weather-column centres. Otherwise rain would stop along a straight line every 512 cells, and temperature would step.
- Weather states, chosen from the values:
  - **Clear:** no precipitation, wind light.
  - **Cloudy:** no precipitation, light shading.
  - **Rain:** rain falls in this weather column, not everywhere at once. Rain wets exposed material, washes soil, puts out fire.
  - **Storm:** heavy rain, strong wind, lightning (see 9.3).
  - **Snow:** precipitation falls as snow in a world column where ambient at that column's surface is below 0 C, and as rain elsewhere. [rev 2.1: rev 2 used ambient at the spawn height, which is the top of the view, so the camera decided rain or snow.]
  - **Fog:** low cloud layer, reduces visibility, rises from water at dawn.

### 9.2 Rain detail [my call, rev 2.2: surface-applied + decorative streaks]
- **Visual vs. simulation:** falling rain in the sky is drawn by the decorative diagonal rain-streak layer from the art style (`2026-11-06-art-style.md`, section 8), clipped at each column's surface so it never appears indoors or underground. The simulation does **not** spawn thousands of 2 x 2 water blocks at the top of the screen, which would look like falling blue gravel next to the thin rain streaks and tie rain to the camera height.
- **Surface application:** each tick, in active columns whose surface cell (section 4.5) lies inside the active region, precipitation is applied directly at that column's top sky-exposed cell (the surface cell, or the highest attached plant / burning cell above it):
  - if surface ambient (section 9.1) is below 0 C, a snow cell is placed on the surface;
  - otherwise, rain wets the cell (or extinguishes a burning cell / flame) and, once the cell is saturated, spawns runoff water on top of it (section 8.8).
- When the camera is deep underground, the column's surface is above the active region, so no rain is applied inside caves. Water from the surface may still seep or run into a cave when the surface is active.
- Rain has a rate (light, medium, heavy) that sets the probability of a hit per column per tick. The rate is limited by the weather column's humidity, and each hit subtracts from it (section 8.8).
- Erosion from rain runs as a random-tick slow process (section 7.4), not per drop.

### 9.3 Lightning [my call]
- During storms, lightning strikes at random points in the weather column.
- A strike runs straight down from the cloud base to the first non-gas cell in that column (the surface, section 4.5). It sets fire to that cell if it is flammable. If the cell is metal, it heats the whole connected metal group.
- Lightning is a single bright frame plus a sound. No damage to anything except material.

### 9.4 Volcanic events (v2)
- Volcanoes erupt on a schedule that is a function of the seed and time. An eruption throws lava and ash upward as particles; lava flows downhill and cools to basalt.
- Eruptions are rare (once per in-game season) so each one is an event.

### 9.5 Tools for events (player-triggered)
Tools sit on the Tools page of the bar (section 3.2).
- **Fire starter:** ignites the cell under the cursor (short flame, uses no resource).
- **Bomb:** places a gunpowder charge that detonates after 3 seconds.
- **Rain cloud:** adds humidity and cloud cover to the weather column under the cursor, so it rains there for a few in-game hours (cooldown of one in-game day).
- **Lightning:** strikes at the cursor (cooldown of one in-game day).
- **Eruption:** starts an eruption at a volcano cell. (v2, with volcanoes.)

All event tools are optional. The sandbox is complete without them. They exist so the player can make things happen without waiting for weather. Cooldowns are saved (section 13).

---

## 10. Building and interacting

- **Painting:** the player paints materials onto cells. Painting is only possible inside the view, which is always inside the active region. [rev 2: rev 1's "painting outside the active area loads the chunk" could never happen.]
- **Replace or fill** [rev 2]: a normal stroke replaces every cell under the brush, including terrain. Alt + stroke only fills cells that are air.
- **Painted state** [rev 2]: a painted cell starts at its material's paint temperature (lava 1,200 C, steam 110 C, ice and snow -10 C, fire at ignition temperature) or, if it has none, at the ambient temperature for that position. Moisture starts at 0, except water-holding materials painted while it is raining, which start at the weather's default. Plants are painted rooted where soil is below, otherwise unattached.
- **Structures:** there is no structure type. A house is built from wood, stone and metal cells painted in shapes. This keeps building open and keeps the physics the only rule.
- **Support:** a painted group of solids that rests on nothing falls as one piece (section 8.7). A player who builds a floating roof sees it collapse, which is the intended lesson.
- **Undo** [rev 2: rev 1 did not define undo in a world that keeps running]: undo covers the last **20 actions or 200,000 changed cells, whichever runs out first**. An action is not a fixed size: a 24-cell brush dragged across the screen is one action and tens of thousands of cells, so counting actions alone does not bound memory.
  - Each action records, per cell, what was there before and what was painted.
  - Undo is a **local revert, not time travel**: the simulation keeps running. A cell is restored only if it still holds what the action painted. Cells the world has changed since (water that flowed away, wood that burned) are skipped.
  - For an exact undo, pause first (Space): nothing moves while paused, so every cell is restored.
  - Undo history is not saved.
- **Brush shapes:** circle, square, line. Line is drawn by holding Shift.

---

## 11. Sound [my call, rev 2]
- **Ambient:** wind level sets a soft noise bed. Rain adds layered droplets, scaled by intensity.
- **Water:** flowing water has a low gurgle; a pool has a soft lap.
- **Fire:** crackle that scales with the number of burning cells.
- **Events:** thunder after lightning, a short boom for explosions, a low rumble for eruptions (v2).
- **Mix:** ambient bed is always on, quiet. Events duck the bed briefly.
- **Mute:** M.
- **Sound follows the simulation's state, through clips** [rev 2]: rev 1 said sounds are "generated from the simulation's state, not from fixed clips." Whether macroquad's audio can play a generated stream has not been verified, and as far as is known it plays loaded clips with volume control. The v1 design therefore uses looped clips (wind bed, light/medium/heavy rain, fire crackle, water) whose volumes are set every frame from simulation counts and rates, plus one-shot clips with random variants for events. The result still follows the world. Fully synthesised sound is checked in M5; if it is wanted and macroquad cannot do it, a dedicated audio crate is evaluated then.

---

## 12. User interface

- **Material bar:** a single row at the bottom with three pages (Materials 1, Materials 2, Tools), switched with Q / E. Each slot is a small swatch with its number key. The page name shows at the left end of the bar.
- **Time bar:** a small line at the top right: day, season, time of day, speed, and "physics limited" or "slowed" when the watchdog acts (section 5.4). Clickable for speed.
- **Hint line:** a single line of text at the bottom-left that shows the current brush and tool. Fades after a few seconds.
- **Settings (Escape):** graphics preset, sound, save, load, seed, quit. The graphics preset sets the budget cap (section 5.4) and particle cap.
- **No other HUD.** No score, no health, no minimap, no debug panels in the release build.

UI follows the art style: muted panels, no glow, small text.

---

## 13. Save and load [rev 2]

**Saved state:**
- world seed and **generator version**;
- `world_tick` and `step_tick` (`u64`, section 5.3);
- camera position;
- every changed chunk, with its surface-heightmap strip;
- weather-column deltas (section 9.1);
- tool cooldowns;
- falling groups (section 8.7).

Not saved: undo history and particles. Before saving, every particle is put back into the grid at its nearest free cell, so nothing is lost.

**Files:** a world folder holds one index file and **region files**, each holding up to 32 x 32 chunks, compressed (LZ4 or similar). [rev 2: rev 1 used one file per chunk. Any chunk that has been active is likely to differ from the generator, because temperature and moisture change wherever it rains or the sun shines, so most visited chunks are saved. That means thousands of chunks after a long session. A raw chunk is 32 KB (64 x 64 x 8 bytes); compressed, mostly-uniform chunks are expected to be a few KB. Region files keep the file count small.] The real size is measured in M4.

**Generator version** [rev 2]: unchanged chunks are not saved; they are regenerated from the seed. If the generator changes, they would regenerate differently and leave seams next to saved chunks. So every world records its generator version, and every released generator version stays in the game, frozen. A world always uses the generator it was created with. Before the first release, generator changes may break old worlds; the game warns instead of loading them silently.

**Safety:**
- Autosave every **5 real minutes** of play, plus on quit. Time is not the trigger: a day is 20 minutes at 1x but 19 seconds at 64x, so an in-game-day autosave would try to write the world every 19 seconds during fast-forward, exactly when the simulation is already using every core.
- The save copies changed chunk data at a step boundary, then hands it to a **single writer thread** that owns all disk writes for both autosave and cache eviction (section 4.2), so eviction and saving never touch the same region file at the same time. [rev 2.1] Each file is written to a temporary name and renamed when complete, so a crash never leaves a half-written save. The previous save is kept until the new one is complete.
- Save files are versioned. The first version of the format is 1.

---

## 14. Testing

### 14.1 Behaviour tests
Every required reaction in section 8.4 has an automated test that:
- builds a small test scene,
- runs the simulation for the stated maximum time (fast reactions in ticks at 20 TPS; slow processes in `world_tick`),
- checks the required result.

For random-tick slow processes [rev 2.3]:
- **Target condition:** at least half ($\ge 50\%$) of a 16-cell strip of the test material reaches the target state within the stated `world_tick` limit.
- **Reproducibility + multi-seed reliability:** CI runs each test with a fixed seed for deterministic regression checks, and a reliability test suite runs each slow-process test across **32 distinct seeds** at **1x ($M=1$), 16x and 64x (including forced $M=16$ and $M=32$)**. A rule is only accepted if all 32 seeds pass at every speed.

A test passing is not enough. Each reaction also needs a manual check: the player plays the scene and confirms it looks right.

**Determinism test** [rev 2, scoped in 2.2]: the same seed, scene, inputs and fixed tick multiplier $M$ give the same world hash after 10,000 ticks, run single-threaded and with 2, 4 and 8 threads.

**Camera-independence test** [rev 2, scoped in 2.2]: the same scene run with the camera at two different heights gives the same result for every cell that stays at least 32 cells inside the intersection of the two active regions, so neither run's margin wall or gas absorption reaches it (section 2, pillar 6).

### 14.2 Performance tests
- A fixed benchmark scene run at two loads: 74,624 active cells (i3 floor) and 154,624 active cells (reference), 600 steps each, plus native fullscreen on each Mac tested.
- Results recorded per platform: average and worst-case frame time.
- **Memory test:** pan continuously for 30 minutes; resident memory stays under the cache cap plus a fixed margin.
- A build is not released if the i3 misses 60 FPS at 1280 x 720.

### 14.3 Soak tests
- 10,000 steps per biome with no crash, no out-of-range temperatures, no cells outside the valid range.
- **Rest test** [rev 2]: generate 1,000 chunks across all v1 biomes and run 600 steps; fewer than 0.1% of cells may move.
- **Save round-trip:** save, load, and compare: the world hash must match.
- **Long-run test:** a year at 64x on a plains-and-lake scene; at the end, the world still has grass, trees and water, and the lake level is within 20% of its start [rev 2.1: checks that evaporation and rain do not slowly drain or flood the world].

### 14.4 Visual checks
- Screenshot diff against the approved reference scenes (section 10 of the art style).

---

## 15. Milestones [rev 2: reordered so nothing is tested before it exists]

Each milestone is a playable build. Each ends with a video, which is also a series episode.

From M0 on, cells are drawn with the art-style palette and 4-px cells, so readability checks in M1-M4 judge something close to the final look. M5 adds lighting, sky, haze and polish. The first terrain is a simple seeded hill function (generator v0); the real generator arrives in M4.

**M0: Foundation.**
- Window, chunk storage, active region with per-cell clipping, camera pan, resident cache (drops unchanged chunks; changed chunks stay in memory until M4).
- Sand, stone and water painted and moving. Four-phase update loop (sequential).
- Renderer check on the i3's GPU, and the measurements in section 5.5.
- Exit: sand piles up on the i3 at 60 FPS; water numbers recorded; decision on whether threads are needed in M1.

**M1: Water and heat.**
- Water flow, steam, lava, ice, basalt. Surface heightmap. Ambient temperature function. Threads, if M0 says so.
- Exit: every water and heat reaction in section 8.4 passes its time limit; determinism and camera-independence tests pass.

**M2: Fire, weather and support.**
- Wood, grass, fire spread, weather field, rain, absorption and runoff, lightning, wind, support groups, explosions and particles.
- Exit: a fire on a dry hillside spreads; rain stops it; wet wood resists; a floating roof falls; a fire panned half off-screen never stops in view.

**M3: Time and seasons.**
- 20 TPS tick clock with fast-forward multiplier, day-night, seasons, random-tick slow processes (`randomTickSpeed`: rot, rust, drying, evaporation, erosion, growth, trees, leaf cycle, soil formation).
- Exit: a wooden hut weathers visibly over one in-game year at 64x on the i3, and the long-run test passes.

**M4: Procedural world.**
- Real generator (v1), plains and mountains, sea and beach, lakes, rest guarantee, region-file saving.
- Exit: the player can pan for a long time in a seeded world, the rest and memory tests pass, and save and reload is exact.

**M5: Vertical slice (the 60-second clip).**
- Art style applied in full (lighting, sky, haze), sound, UI, audio check (section 11).
- Exit: the target scene (house on a hillside through rain, fire and seasons) plays for 60 seconds on the i3 and looks like the approved frames.

**M6: More biomes and events.**
- Desert, volcano, island, forest, shore erosion, eruptions, remaining event tools.

**M7: Release preparation.**
- Settings, save versioning and generator freezing, packaging for Mac, Windows and Linux, a short README.

Each milestone is a separate design-to-build cycle: the plan for that milestone is written and approved before its code.

---

## 16. Series plan (YouTube and Rust learning) [my call]

- The series follows the milestones. One milestone = one or two episodes.
- Each episode starts from the previous milestone's git tag, so viewers can follow along from a working point.
- Each episode opens with the result (a short clip of what the milestone does), then breaks down the Rust needed.
- Rust concepts arrive in the order the project needs them: ownership with the chunk storage (M0), enums and matching for materials (M0-M1), traits and generics for the behaviour rules (M1-M2), threads for the four-phase update (M1 or M2, whichever milestone first needs them, section 5.8), file I/O and compression for saves (M4).
- Code stays readable. No clever tricks a beginner cannot follow, even if they run slightly faster.

---

## 17. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| i3 cannot reach 60 FPS with water-heavy scenes | Game unusable on the target machine | Measure in M0 (section 5.5). Dirty rectangles, threads, then a smaller view cap before adding features. |
| macroquad does not run on the i3's integrated GPU | Floor machine unusable | Check in M0 before anything else (section 5.5). |
| Fire fronts stop at the margin | Visible bug | Larger margin for fire; resume on camera return. Tested in M2. |
| Support flood fills are expensive when the player digs under large structures | Frame spikes | Fill bounded at 4,096 cells; only on change; measured in M2. |
| Physics looks slow relative to the clock at 16x and 64x | Odd look during fast-forward | Accepted (section 5.3). Fast-forward is for seasons. |
| Required reactions look wrong because numbers are real | Anti-goal | Tune for visibility, not realism. The test table is the spec. |
| Generator changes break saved worlds | Seams in old worlds | Generator version saved; released generators frozen (section 13). |
| Scope creep: too many biomes too early | Never finishes | Milestones are fixed; new biomes wait for M6. |
| Tree regrowth and renewal add scope to M3 | Delay | Simple templates; if M3 runs long, trees move to M6 and grass alone carries renewal. |
| Water balance drifts over a long run (lakes dry up or flood) | Dead or flooded world | Humidity delta decays toward baseline (section 8.8); checked by the long-run test in M3 (section 14.3). |
| Rust learning slows the project | Delays | Keep code simple; accept slower progress as part of the series. |
| Procedural sound not possible in macroquad | Sound less alive | Clip mixing driven by simulation (section 11). |
| Save corruption | Lost worlds | Versioned format; write-then-rename; keep the previous save until the new one is complete. |

---

## 18. Open questions

These need the user's answer before the plan is final. Most important first.

1. **Target hardware:** is a first-generation Core i3 (2010) with 2-4 GB RAM the minimum? Which exact model is the test machine, and do you own it? Section 5 is built on measuring that machine. If the minimum is higher, the budget grows.
2. **Zoom (section 3.3):** rev 2 removes zoom, because the fixed 4-px cell and the art style make it impossible. Is that acceptable? If zoom is wanted, choose what gives: (a) integer cell scaling for zoom-in only (8 px, 16 px), which breaks the art style's "never draw a cell larger" rule; or (b) no zoom.
3. **Time scale:** is 20 real minutes per in-game day right? If the user wants faster seasons for the video, the clock in section 6.1 changes.
4. **Event tools (section 9.5):** should the player have fire, bombs, rain clouds and lightning from the start, or should some wait until later milestones?
5. **Undo (section 10):** is a local revert of the last 20 actions / 200,000 cells right?
6. **Controls (section 3.2):** keys are a proposal. Confirm or change.
7. **Platform priority:** Mac first, then Windows and Linux, or all at once?
8. **Series pace:** one video per milestone, or one per week?
9. **Physical cell scale (section 4.1):** does a cell have a real-world size, or is that left to tuning in M1?
10. **Scope changes in rev 2:** tides are removed, waves are drawn only, shore erosion is v2, lamps are removed from v1, and tree regrowth is in v1 (M3). Confirm.
11. **Sound (section 11):** is clip mixing driven by the simulation acceptable for v1, instead of fully synthesised sound?

**Decided, no longer open:**
- Frozen off-screen for v1 (section 5.2). Revisit later if aging off-screen is wanted.

---

## 19. Approval

When the user approves this document, the next step is the plan for milestone M0 (`superpowers:writing-plans`), not code.
