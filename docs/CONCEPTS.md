# gsplat-wasm — Concepts & Methods (a primer)

A from-scratch explanation of the ideas behind this project: what each term means, why it
matters, and how we actually used it. Written to be read top-to-bottom, but every section
stands alone, and there's a [glossary](#glossary) at the end. It grows as the project does —
when we add a concept to the code, we add it here.

The one idea to anchor everything: **instead of building a 3D object out of a hard triangle
shell with a painted skin, a Gaussian splat represents it as a cloud of thousands of tiny,
soft, coloured blobs floating in space.** Look at the cloud from any angle and the blobs
overlap into a convincing solid. Our whole job is: turn **photos** into such a cloud, and let
it be **viewed and shared**, all **inside a web browser** with no install and no server.

- **Splat** = the cloud-of-blobs representation (each blob is one *3D Gaussian*).
- **Browser-native** = it runs as a static web page, using the GPU through **WebGPU**, with the
  program written in **Rust** and compiled to **WebAssembly**.

Almost everything below is either "what a splat *is* and how it's drawn," or "how we get a GPU
program running inside a browser tab," plus the math and the colour bookkeeping that connect them.

---

## 1. Splat vs. mesh — the core distinction

The sibling tool (`lo_fi_converter_blender_addon`) works with **meshes**: a 3D object as a shell
of **triangles** (points joined into a surface) wearing a **texture** (a painted image). That's
the dominant way games represent geometry.

A **Gaussian splat** is a completely different representation — a **radiance field** sampled as
a point cloud:

- There is **no surface, no triangles, no texture**. There is a big list of **3D Gaussians**.
- Each Gaussian is a fuzzy ellipsoidal blob: a position in space, a size/shape, a colour, and a
  transparency. Think of an airbrush dot, or a soft cotton ball, sitting in 3D.
- Tens of thousands to millions of these, overlapping, *add up* to look like a real object —
  the way a pointillist painting resolves into a scene when you step back.

**Why use this instead of a mesh?** Two reasons that matter for us:

1. **It comes straight from photos.** You can *optimize* a splat to match a set of photographs
   (see §9) far more directly than you can reconstruct a clean triangle mesh. The output looks
   photographic, including soft/fuzzy things (hair, foliage) that meshes struggle with.
2. **Its heavy compute is GPU-shaped.** Drawing and training splats is massively parallel
   arithmetic over points — exactly what a GPU is for. And the one accelerator a web page can
   touch is the GPU, via **WebGPU**. (Classical photogrammetry, by contrast, is CPU-bound and
   hostile to the browser — that's *why* this project pivoted to splatting; see `DESIGN.md` §2.)

> **Mental model:** a mesh is a *carved statue with paint*; a splat is a *3D cloud of spray-paint
> mist* that happens to look solid. We're building the cloud, not the statue. (A statue/mesh can
> be *extracted* from a splat later — but that's a separate, offline step; `DESIGN.md` §6.)

---

## 2. The pipeline, stage by stage

The end goal is **photos → a viewable splat**, entirely client-side. The assembly line:

```
photos → [camera poses] → [train the splat] → [view / share]
            (SfM, CPU)       (Brush, GPU)        (viewer, GPU)
```

| # | Stage | What it does | Where it runs | Status |
|---|-------|--------------|---------------|--------|
| 1 | **Pose** | Work out where each photo was taken from (§8) | browser, CPU (wasm) | future (Phase 3) |
| 2 | **Train** | Optimize a splat to match the posed photos (§9) | browser, GPU (WebGPU) | future (Phase 2) |
| 3 | **View** | Render the splat, let you orbit/zoom it (§4) | browser, GPU (WebGPU) | **done (Phase 1)** |

We built **stage 3 first** because it's the lowest-risk visible win and it proves the whole
browser+GPU+hosting toolchain works before we bet on the harder stages. To have something to
view without yet having stages 1–2, Phase 1 **generates a splat procedurally** (a rainbow
sphere with red/green/blue axes — see `crates/gsplat-app/src/scene.rs`) instead of training one.
That sample exercises every part of the real viewer; a real `.ply`/`.spz` file just produces the
same kind of Gaussian list from a different source.

---

## 3. Anatomy of one Gaussian

Everything hinges on what a single blob is. In code it's the `Gaussian` struct (from
`wgpu-3dgs-core`), and it has exactly five pieces:

```rust
struct Gaussian {
    pos:   Vec3,        // where the blob's centre sits in 3D space
    scale: Vec3,        // how big it is along its 3 local axes (a stretch per axis)
    rot:   Quat,        // how that ellipsoid is oriented (a rotation)
    color: U8Vec4,      // its colour (red, green, blue, + alpha) as 0–255 bytes
    sh:    [Vec3; 15],  // optional view-dependent colour tweak (spherical harmonics)
}
```

### 3.1 Position, scale, rotation → the ellipsoid (the *covariance*)

- **`pos`** is just the centre point (the *mean* of the Gaussian, often written **μ**).
- **`scale`** is three numbers: how far the blob spreads along its own x, y, z. Equal numbers →
  a round ball; unequal → a stretched ellipsoid (a "lozenge" or a flat "disc").
- **`rot`** orients that ellipsoid. It's a **quaternion** — a compact, gimbal-lock-free way to
  store a 3D rotation as four numbers `(x, y, z, w)` (think "an axis to spin around + how much").

Mathematically these three combine into one 3×3 matrix called the **covariance, Σ**, via
**Σ = R · S · Sᵀ · Rᵀ** (R = rotation matrix from the quaternion, S = the diagonal scale matrix).
Σ *is* the ellipsoid: it says how the blob's density spreads out in every direction. A "Gaussian"
in statistics is the bell curve; a **3D Gaussian** is a 3D bell-shaped lump of density centred at
`pos`, whose spread is Σ. The blob is densest (most opaque) at the centre and fades out toward
the edges.

> **Why store scale+rotation instead of Σ directly?** Because an arbitrary 3×3 matrix can be an
> invalid (non-ellipsoid) shape. Storing a rotation and three positive scales *guarantees* a
> valid ellipsoid, and is what the training math (§9) optimizes cleanly. Our code keeps `scale`
> in **linear** units (a real size like 0.014), and the GPU builds Σ from it directly — no
> hidden exp/log on our side. (Splat **files** often store the *log* of the scale; the loader
> exponentiates it back to linear on the way in. Worth knowing so a hand-built splat isn't
> 1000× too big.)

### 3.2 Colour and opacity

- **`color`** is plain linear **RGBA** in bytes (0–255). RGB is the blob's colour; **A (alpha)**
  is its **opacity** — how solid vs. see-through it is. Opacity is central: the final image is
  built by stacking many semi-transparent blobs (§4.2), so a blob's alpha controls how much it
  contributes versus what's behind it.

### 3.3 Spherical harmonics (`sh`) — colour that changes with viewing angle

Real surfaces don't look the same colour from every direction (a shiny spot, a glint, a
colour-shifting sheen). Splats capture that by letting a blob's colour **depend on the direction
you view it from**. The tool for "a function defined over all directions on a sphere" is
**spherical harmonics (SH)** — a set of basis patterns (constant, then lobes of increasing
complexity) you can mix with coefficients, exactly like a Fourier series but wrapped onto a
sphere.

- **Degree 0** = the constant term (the "**DC**" colour) — one value, the base colour regardless
  of angle. In our struct that base lives in `color`.
- **Degrees 1–3** add directional variation: 3, then 5, then 7 more coefficients per channel
  (15 extra total for degree 3) — that's the `sh: [Vec3; 15]`.

Our synthetic sample sets `sh` to all-zero (degree 0 only), so every blob is a flat colour from
all sides — simplest possible, and plenty to validate the renderer. Trained splats use the higher
degrees for realism.

---

## 4. How a splat actually gets drawn

We hand the viewer a list of Gaussians; it produces pixels every frame. Three conceptual steps,
which in `wgpu-3dgs-viewer` are three real components (a **Preprocessor**, a **RadixSorter**, and
a **Renderer**).

### 4.1 Project each 3D blob to a 2D ellipse (Preprocessor)

The camera (§6) defines how 3D space maps to the screen. Each 3D Gaussian, viewed through that
camera, lands as a **2D Gaussian** — an elliptical smudge — on screen. The 3D ellipsoid Σ becomes
a 2D ellipse via the projection's local linearization (the **Jacobian**); this is the classic
**EWA splatting** step. The preprocessor also **culls** blobs that fall off-screen (so we don't
waste work) and records each visible blob's screen position, 2D shape, colour, and depth.

### 4.2 Sort the blobs by depth (RadixSorter) — and *why order matters*

The image is composited by drawing the semi-transparent ellipses **on top of each other** and
blending. Blending is **order-dependent**: "draw a faint red over blue" ≠ "draw blue over faint
red." To look right, blobs must be drawn in **depth order** (back-to-front, the *painter's
algorithm*). Every frame the set of visible blobs and their depths change as the camera moves, so
**every frame we re-sort all of them** by distance from the camera.

That sort is done on the GPU with a **radix sort** — a fast, non-comparison sort that orders
numbers by processing their digits/bits in passes. It's used here because it parallelizes well
across tens of thousands of blobs on the GPU.

### 4.3 Rasterize the ellipses and blend (Renderer)

Finally each 2D ellipse is drawn as a little screen quad; for every pixel it covers, a fragment
shader evaluates the Gaussian falloff (opaque at the centre, fading out) times the blob's colour
and alpha, and **alpha-blends** it over what's already there. Stack thousands in depth order →
the solid-looking object. The viewer can also draw blobs as plain ellipses or points for
debugging, but **Splat** mode is the real one.

> **Mental model of the whole frame:** *project* every blob to a screen smudge → *sort* the
> smudges far-to-near → *paint* them in that order, each slightly see-through. Repeat 60×/sec.

---

## 5. Getting a GPU program to run in a browser tab

This is the half of the project that's about *delivery*, not splats. A web page can't normally
run a Rust GPU program. Here's the stack that makes it possible, top to bottom.

### 5.1 WebAssembly (WASM)

**WebAssembly** is a portable binary instruction format that browsers run at near-native speed,
in the same sandbox as JavaScript. We write the app in **Rust** and compile it to the
**`wasm32-unknown-unknown`** target — a 32-bit WASM with no operating system underneath (no files,
no threads by default, a 4 GB address ceiling). The browser downloads the `.wasm` and runs it.

### 5.2 WebGPU — the browser's modern GPU API

**WebGPU** is the new standard browser API for talking to the GPU (the successor to WebGL). Unlike
WebGL it exposes **compute shaders** (general GPU programs, needed for our preprocessing and
sorting) as well as rendering. It's the one piece of real acceleration a static page can use, and
it's why splatting is feasible in the browser at all.

### 5.3 `wgpu` — WebGPU for Rust (and beyond)

**`wgpu`** is a Rust library implementing the WebGPU API. In the browser it forwards to the
browser's WebGPU; compiled natively it translates the *same code* to the OS GPU API (Metal on Mac,
Vulkan, or DX12). That's why our `graphics` module compiles and runs both on the web *and*
natively — handy for fast local checks. Core `wgpu` nouns you'll see:

- **Instance** → entry point; **Adapter** → a specific GPU; **Device** + **Queue** → the open
  connection you create resources on and submit work to.
- **Surface** → the drawable region tied to the page's canvas — really a **swapchain** of textures
  you draw into and then **present**. You **configure** it with a size and pixel **format**.
- **CommandEncoder** → records GPU commands; you `submit` them to the queue each frame.

### 5.4 winit — window, canvas, and the event loop

**`winit`** is a cross-platform windowing library. On the web it manages an **HTML `<canvas>`**
(the page element WebGPU draws into) and an **event loop** that delivers input (mouse, keyboard,
resize) and "time to draw a frame" (`RedrawRequested`) callbacks. Our `app` module implements
winit's `ApplicationHandler`: create the window/canvas, then on each redraw update the camera and
render. We request the next frame after each one (`request_redraw`) to animate continuously.

A web-specific wrinkle: creating the GPU **Device is asynchronous**, and a browser tab **cannot
block** waiting for it. So instead of blocking, we kick off the async setup with
`wasm_bindgen_futures::spawn_local` and start rendering once it's ready. (Natively we *can* block,
so we just do — one of a few small `#[cfg]` splits between web and native.)

### 5.5 trunk + wasm-bindgen — the build and the JS glue

WASM and JavaScript can't call each other directly without generated bindings.

- **`wasm-bindgen`** generates the glue: JS functions that call into our WASM and back, plus the
  `web-sys`/`js-sys` bindings that let Rust call browser APIs (DOM, canvas, …). The
  `#[wasm_bindgen(start)]` function is our entry point, run automatically when the page loads.
- **`trunk`** is the bundler/dev-server. It runs `cargo build`, then `wasm-bindgen`, then writes an
  `index.html` that loads everything, and serves it with **live reload** (edit a file → it
  rebuilds → the page refreshes). `trunk serve` is our dev loop; `trunk build` produces the static
  `dist/` we'd deploy.

> **Gotcha worth internalizing:** wasm-bindgen's glue format is **version-locked** — the
> `wasm-bindgen` *library* compiled into the WASM must exactly match the `wasm-bindgen` *CLI* tool
> that post-processes it. Mismatch = hard error. (We hit this; see §10.)

---

## 6. The camera and the matrices

To turn 3D blobs into a 2D image you need a **camera**: a position, an orientation, and a lens.
The viewer's `Camera` stores `pos`, a look direction as **pitch** + **yaw** angles, a vertical
**field of view**, and a near/far range.

- **Yaw / pitch** are two of the three "Euler angles": **yaw** = turn left/right, **pitch** =
  look up/down. From them you compute a unit **forward** vector. (The third, *roll*, we don't use.)
- **Field of view (FOV)** = how wide the lens is, in degrees — a bigger FOV sees more but with more
  perspective distortion (60° here, a normalish lens).
- **Near / far planes** = the closest and farthest distances the camera renders; everything outside
  is clipped. (They also set the precision of the depth buffer.)

Two matrices do the actual 3D→2D:

- The **view matrix** moves the world *into the camera's frame* ("put the camera at the origin
  looking down −Z"). Built here with `look_to_rh(eye, forward, up)` — *rh* = right-handed coords.
- The **projection matrix** applies the lens: perspective foreshortening from FOV + **aspect
  ratio** (width/height) + near/far. Built with `perspective_rh(fov, aspect, near, far)`.

Multiply a 3D point by view then projection and you land in **clip space**, which the GPU turns
into normalized device coordinates and then pixels. **Aspect ratio is why viewport size matters** —
feed the wrong size and everything stretches or projects wrong (this caused a real bug; §10).

### 6.1 Orbit camera (what Phase 1 uses)

The viewer's camera is a free-fly (first-person) camera. A **model viewer** wants an **orbit**
camera instead: the object stays put and you circle it. We build orbit *on top of* the free-fly
camera (no fork) in `camera_control.rs`: keep a **target** point, a **distance**, and two orbit
angles; each frame compute the forward direction from the angles and place the camera at
`target − forward × distance`, looking inward. Drag changes the angles, scroll changes the
distance, and a gentle idle auto-spin makes the 3D obvious at a glance.

---

## 7. Colour and pixels (small things that look wrong if ignored)

- **Linear vs. sRGB colour.** Displays expect **sRGB** (a gamma-encoded space tuned to human
  vision), but blending/lighting math is only correct in **linear** space. Mix them up and the
  image looks washed-out or too dark. The fix here: the viewer renders into a **non-sRGB view** of
  the surface (linear math), and the surface itself applies the sRGB encode on present. That's the
  `remove_srgb_suffix()` you see in the surface config.
- **Device pixel ratio (DPR).** A "pixel" in CSS isn't a physical pixel. On a Retina display
  `devicePixelRatio` is 2 — each CSS pixel is a 2×2 block of real pixels. To render crisply we size
  the GPU **drawing buffer** to `CSS size × DPR`, while the canvas is *displayed* at the CSS size.
  Skip this and the image is soft/blurry.

---

## 8. Pose estimation / Structure-from-Motion (the deferred hard part)

Training a splat (§9) needs to know, for each photo, **where the camera was** — its **pose**.
Recovering camera poses (and a rough 3D point cloud) from a set of overlapping photos is
**Structure-from-Motion (SfM)**, classical computer vision. It's the project's biggest open risk
and isn't built yet (Phase 3), but the vocabulary:

- **Intrinsics** = a camera's *internal* geometry: focal length, principal point (where the optical
  axis hits the sensor). **Extrinsics** = its *external* pose: rotation + translation in the world.
  SfM recovers both.
- **Features & descriptors** — distinctive points (corners, blobs) detected in each image (e.g.
  **AKAZE**), each summarized by a **descriptor** vector so the *same* world point can be
  recognized across photos.
- **Matching** — pair up features that describe the same point between images.
- **RANSAC** — matches are noisy; RANSAC robustly fits the geometry while ignoring the wrong
  matches (outliers).
- **P3P / PnP** — solve a camera's pose from known 3D points and their 2D projections.
- **Triangulation** — given a point seen in two known cameras, intersect the rays to get its 3D
  position (the sparse point cloud).
- **Bundle adjustment** — a big nonlinear least-squares polish that jointly nudges *all* camera
  poses and 3D points to minimize total reprojection error.

We keep this behind a **`PoseEstimator` trait** (`crates/gsplat-core`) so the backend can be
swapped — classical CPU SfM now, a machine-learning poser later if one ever fits the browser's
limits (`DESIGN.md` §4 explains why none do today).

---

## 9. Training a splat (the ML, also deferred)

This is the headline ML step (Phase 2), done in the browser via **Brush** (built on **Burn**, a
Rust ML framework, over `wgpu`). The idea is **differentiable rendering + gradient descent**:

1. Start from a rough cloud of Gaussians (seeded from SfM's sparse points).
2. Render it from a known camera pose and compare to the *real* photo from that pose — the
   difference is the **loss**.
3. Because the renderer is **differentiable**, you can compute how to nudge every Gaussian's
   position/shape/colour/opacity to make the render look *more* like the photo (**backpropagation**
   / gradient descent), and take a small step.
4. Repeat over all photos, thousands of times. Periodically **densify** (split/clone blobs where
   detail is missing, prune useless ones).

The result is a splat that reproduces the photos from every captured angle. "Differentiable" is the
magic word: it turns "make a 3D model" into "minimize an error by calculus," which GPUs do fast.

---

## 10. The hard lessons (bugs that taught the most)

The most instructive moments so far were the wasm/GPU integration bugs. Each is a real principle.

- **WASM is single-threaded, but the type system still needs convincing.** Rust's **`Send`/`Sync`**
  marker traits mean "safe to move/share across threads." A viewer dependency returned a future
  declared `+ Send`, but `wgpu`'s handles are deliberately **`!Send`/`!Sync`** on wasm — so it
  wouldn't compile, even though wasm has no threads for the bound to matter. Fix: enable wgpu's
  **`fragile-send-sync-non-atomic-wasm`** feature, which makes its wasm handles claim Send/Sync
  (sound precisely *because* there are no threads). **Lesson:** on wasm, threading bounds are
  vacuous but still load-bearing in the compiler; this feature is the standard opt-in.

- **"Plain old data" depends on memory layout (`+simd128`).** To upload a struct to a GPU buffer
  you reinterpret it as raw bytes — only legal if the type is **`Pod`** (*Plain Old Data*: no
  padding, no pointers, every bit pattern valid), checked by the **`bytemuck`** crate. The math
  type `glam::Vec3A` is only `Pod` in its **SIMD** form, which on wasm requires enabling
  **WebAssembly SIMD** (`+simd128`, set in `.cargo/config.toml`). Without it: "the trait `Pod` is
  not satisfied." **Lesson:** whether a type is byte-castable can hinge on a target feature; SIMD
  also happens to make it faster.

- **On the web, the canvas size lies — reconcile it every frame.** Our first render was a
  full-screen colour smear. Cause: the GPU surface was configured at a stale, tiny size (winit's
  `inner_size()` isn't reliable right after web startup), so the viewer projected every blob as if
  the screen were a few pixels across — each smeared over the whole canvas. Fix: in `render`, read
  the canvas's real CSS size × DPR and reconfigure the surface whenever it changes
  (`graphics::drawable_size`). **Lesson:** don't trust a one-time size on the web; the
  authoritative size is the live canvas, checked each frame.

- **wasm-bindgen's ABI is version-locked, and tooling URLs rot.** `trunk` tried to fetch a
  `wasm-bindgen` CLI matching our crate (0.2.126), but its built-in downloader pointed at a moved
  GitHub org and 404'd, and no prebuilt binary for that version existed. Fix: install the matching
  CLI from source so the on-PATH tool exactly matches the crate. **Lesson:** the WASM↔JS glue
  format is unstable across versions; the crate and CLI versions must match exactly, and
  auto-download isn't guaranteed.

- **Single-threaded WebGPU needs no special hosting (`crossOriginIsolated`).** Wasm *threads* use
  `SharedArrayBuffer`, which browsers only allow when the page is **cross-origin isolated** — which
  requires **COOP + COEP** HTTP headers. Our viewer is single-threaded (WebGPU does the parallelism
  on the GPU), uses no SharedArrayBuffer, and so needs **none** of that — it hosts as plain static
  files (incl. GitHub Pages). **Lesson:** only reach for COOP/COEP if/when we adopt a wasm-threads
  path (Brush training might); don't pre-impose it.

---

## 11. The methods, named (for further reading)

| What we call it | The real technique / source |
|---|---|
| Gaussian splatting | **3D Gaussian Splatting for Real-Time Radiance Field Rendering** (Kerbl et al., 2023) |
| Blob → screen ellipse | **EWA (Elliptical Weighted Average) splatting** (Zwicker et al., 2001) |
| Ellipsoid from rot+scale | **Covariance factorization** Σ = R S Sᵀ Rᵀ |
| View-dependent colour | **Spherical harmonics** lighting/appearance basis |
| Depth ordering | **Painter's algorithm** + **GPU radix sort** |
| Transparency compositing | **Alpha blending / the "over" operator** (Porter–Duff) |
| Camera projection | **View / perspective-projection matrices**; right-handed clip space |
| Rotation storage | **Quaternions** (gimbal-lock-free 3D rotation) |
| Pose recovery | **Structure-from-Motion**: features (**AKAZE**), **RANSAC**, **PnP/P3P**, **bundle adjustment** |
| Splat training | **Differentiable rendering** + gradient descent; **Brush** on **Burn**/wgpu |
| Browser GPU | **WebGPU**; **`wgpu`** (Rust); compute + render pipelines |
| Rust → web | **WebAssembly** (`wasm32-unknown-unknown`), **wasm-bindgen**, **trunk** |
| Byte-casting for GPU | **`bytemuck` / `Pod`**; **WebAssembly SIMD** (`+simd128`) |
| Colour correctness | **Linear vs. sRGB** colour space; **device pixel ratio** |
| Cross-origin isolation | **COOP/COEP**, **SharedArrayBuffer**, `crossOriginIsolated` (only for wasm threads) |
| File formats | **`.ply`** (point cloud), **`.spz`** (compressed splat) |

---

## Glossary

- **Adapter / Device / Queue (wgpu)** — a GPU / your open connection to it / the channel you submit
  work on.
- **Alpha / opacity** — how solid (vs. see-through) a blob is; drives blending.
- **Alpha blending** — combining semi-transparent layers; order-dependent, so splats are depth-sorted.
- **AKAZE** — a feature detector/descriptor used in SfM to find and describe image keypoints.
- **Aspect ratio** — viewport width ÷ height; wrong value distorts the projection.
- **Bundle adjustment** — joint nonlinear least-squares refinement of all camera poses + 3D points.
- **`bytemuck` / `Pod`** — Rust crate / trait for safely reinterpreting a struct as raw bytes (for GPU upload).
- **Canvas** — the HTML element WebGPU draws into.
- **Covariance (Σ)** — the 3×3 matrix encoding a Gaussian's ellipsoid (size + orientation).
- **COOP / COEP** — HTTP headers that make a page *cross-origin isolated*; needed only for SharedArrayBuffer / wasm threads.
- **Device pixel ratio (DPR)** — physical pixels per CSS pixel (2 on Retina); size the draw buffer by it.
- **Differentiable rendering** — a renderer you can backpropagate through, enabling splat training.
- **EWA splatting** — projecting a 3D Gaussian to a 2D screen ellipse via the projection Jacobian.
- **Extrinsics / Intrinsics** — a camera's pose in the world / its internal lens geometry.
- **Field of view (FOV)** — how wide the camera lens is, in degrees.
- **Gaussian (3D)** — one fuzzy ellipsoidal blob: position, scale, rotation, colour, opacity.
- **Linear vs. sRGB** — math-correct colour space vs. display-encoded colour space.
- **Painter's algorithm** — draw far-to-near so nearer things cover farther ones.
- **Pose** — where/how a camera was positioned when a photo was taken.
- **Projection matrix** — applies the lens (FOV, aspect, near/far): camera space → clip space.
- **Quaternion** — a four-number, gimbal-lock-free representation of a 3D rotation.
- **Radix sort** — a fast non-comparison sort; used on the GPU to depth-order splats each frame.
- **RANSAC** — robust model fitting that ignores outlier matches.
- **Spherical harmonics (SH)** — a basis for functions over directions; lets a blob's colour vary with view angle.
- **Splat** — the point-cloud-of-Gaussians representation (and, loosely, one such blob).
- **Structure-from-Motion (SfM)** — recovering camera poses + sparse 3D points from photos.
- **Surface / swapchain (wgpu)** — the canvas-backed textures you render into and present.
- **`Send` / `Sync`** — Rust markers for thread-safe move/share; vacuous on single-threaded wasm but still type-checked.
- **`+simd128`** — the WebAssembly SIMD target feature (needed for glam `Vec3A: Pod`; also faster).
- **trunk** — the WASM web bundler + live-reload dev server we build the site with.
- **View matrix** — moves the world into the camera's frame: world space → camera space.
- **WebAssembly (WASM)** — portable binary the browser runs near-natively; our Rust compiles to it.
- **WebGPU** — the modern browser GPU API (compute + render); the acceleration a static page can use.
- **`wgpu`** — the Rust library implementing WebGPU (web) and native GPU backends (Metal/Vulkan/DX12).
- **wasm-bindgen** — generates the JS↔WASM glue; its ABI is version-locked to its CLI.
