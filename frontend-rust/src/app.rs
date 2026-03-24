use leptos::prelude::*;
use std::cell::Cell;
use wasm_bindgen::JsCast;
use web_sys::WebGlRenderingContext as GL;

use crate::components::Header;
use crate::state::AppState;
use crate::worker_bridge::{init_worker, send_to_worker, terminate_worker};
use crate::worker_protocol::{MainToWorker, WorkerToMain};

const STYLES: &str = include_str!("styles.css");

const DOOM_WIDTH: usize = 640;
const DOOM_HEIGHT: usize = 400;
const BUFFER_SIZE: usize = DOOM_WIDTH * DOOM_HEIGHT * 4;

const VERT_SHADER: &str = r#"
    attribute vec2 a_pos;
    varying vec2 v_uv;
    void main() {
        gl_Position = vec4(a_pos, 0.0, 1.0);
        v_uv = (a_pos + 1.0) / 2.0;
    }
"#;

const FRAG_SHADER: &str = r#"
    precision mediump float;
    varying vec2 v_uv;
    uniform sampler2D u_tex;
    void main() {
        // Flip Y for WebGL (bottom-up) to match canvas (top-down)
        gl_FragColor = texture2D(u_tex, vec2(v_uv.x, 1.0 - v_uv.y));
    }
"#;

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    set_test_state(state);

    Effect::new(move || {
        thread_local! { static INIT: Cell<bool> = const { Cell::new(false) }; }
        INIT.with(|init| {
            if init.get() { return; }
            init.set(true);

            let subscribed = std::cell::Cell::new(false);
            let result = init_worker(move |msg| {
                match msg {
                    WorkerToMain::Connected => {
                        state.status.set(crate::state::ConnectionStatus::Connected);
                        state.status_message.set("Connected".to_string());
                        if !subscribed.get() {
                            subscribed.set(true);
                            send_to_worker(MainToWorker::Subscribe { chunk_ids: vec![] });
                        }
                    }
                    WorkerToMain::FatalError { message } => {
                        state.status.set(crate::state::ConnectionStatus::Error);
                        state.status_message.set(message);
                    }
                    _ => {}
                }
            });

            if let Err(e) = result {
                web_sys::console::error_1(&format!("Worker init failed: {}", e).into());
                return;
            }

            let uri = get_spacetimedb_uri();
            let closure = wasm_bindgen::closure::Closure::once(move || {
                send_to_worker(MainToWorker::Connect {
                    uri,
                    database: "doom-checkboxes".to_string(),
                });
            });
            web_sys::window().unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(), 100).ok();
            closure.forget();
        });
    });

    on_cleanup(|| { terminate_worker(); });

    view! {
        <style>{STYLES}</style>
        <Header state=state />
        <DoomCanvas state=state />
    }
}

/// WebGL-accelerated Doom canvas
#[component]
fn DoomCanvas(state: AppState) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // Initialize WebGL once, then re-upload texture on each render version bump
    Effect::new(move |_| {
        let _version = state.render_version.get();

        GL_STATE.with(|gl_state| {
            let mut gs = gl_state.borrow_mut();

            // Lazy init WebGL on first render
            if gs.is_none() {
                if let Some(canvas) = canvas_ref.get() {
                    let canvas: &web_sys::HtmlCanvasElement = &canvas;
                    *gs = init_webgl(canvas);
                }
            }

            let Some(ref gl_s) = *gs else { return };

            // Upload pixel buffer as texture and draw
            PIXEL_BUFFER.with(|buf| {
                let buf = buf.borrow();
                let gl = &gl_s.gl;

                gl.bind_texture(GL::TEXTURE_2D, Some(&gl_s.texture));

                unsafe {
                    let arr = js_sys::Uint8Array::view(&buf[..]);
                    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                        GL::TEXTURE_2D, 0, GL::RGBA as i32,
                        DOOM_WIDTH as i32, DOOM_HEIGHT as i32, 0,
                        GL::RGBA, GL::UNSIGNED_BYTE, Some(&arr),
                    ).ok();
                }

                gl.draw_arrays(GL::TRIANGLES, 0, 6);
            });
        });
    });

    view! {
        <canvas
            node_ref=canvas_ref
            width=DOOM_WIDTH
            height=DOOM_HEIGHT
            style="image-rendering: pixelated; width: 100%; max-width: 960px; display: block; margin: 20px auto; background: #000;"
        />
    }
}

struct GlState {
    gl: GL,
    texture: web_sys::WebGlTexture,
}

thread_local! {
    static GL_STATE: std::cell::RefCell<Option<GlState>> = const { std::cell::RefCell::new(None) };
}

fn init_webgl(canvas: &web_sys::HtmlCanvasElement) -> Option<GlState> {
    let gl: GL = canvas.get_context("webgl").ok()??.dyn_into().ok()?;

    // Compile shaders
    let vs = compile_shader(&gl, GL::VERTEX_SHADER, VERT_SHADER)?;
    let fs = compile_shader(&gl, GL::FRAGMENT_SHADER, FRAG_SHADER)?;

    let program = gl.create_program()?;
    gl.attach_shader(&program, &vs);
    gl.attach_shader(&program, &fs);
    gl.link_program(&program);
    gl.use_program(Some(&program));

    // Full-screen quad
    let verts: [f32; 12] = [-1.0,-1.0, 1.0,-1.0, -1.0,1.0, -1.0,1.0, 1.0,-1.0, 1.0,1.0];
    let buf = gl.create_buffer()?;
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buf));
    unsafe {
        let arr = js_sys::Float32Array::view(&verts);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &arr, GL::STATIC_DRAW);
    }

    let a_pos = gl.get_attrib_location(&program, "a_pos") as u32;
    gl.enable_vertex_attrib_array(a_pos);
    gl.vertex_attrib_pointer_with_i32(a_pos, 2, GL::FLOAT, false, 0, 0);

    // Create texture
    let texture = gl.create_texture()?;
    gl.bind_texture(GL::TEXTURE_2D, Some(&texture));
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);

    gl.viewport(0, 0, DOOM_WIDTH as i32, DOOM_HEIGHT as i32);

    web_sys::console::log_1(&"WebGL initialized for Doom rendering".into());

    Some(GlState { gl, texture })
}

fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> Option<web_sys::WebGlShader> {
    let shader = gl.create_shader(shader_type)?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl.get_shader_parameter(&shader, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
        Some(shader)
    } else {
        web_sys::console::error_1(&format!("Shader error: {}",
            gl.get_shader_info_log(&shader).unwrap_or_default()).into());
        None
    }
}

fn get_spacetimedb_uri() -> String {
    let window = web_sys::window().expect("no window");
    let hostname = window.location().hostname().unwrap_or_default();
    if hostname == "localhost" || hostname == "127.0.0.1" {
        "ws://127.0.0.1:3000".to_string()
    } else {
        "wss://maincloud.spacetimedb.com".to_string()
    }
}

// === Pixel buffer + rendering ===

thread_local! {
    pub static PIXEL_BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; BUFFER_SIZE]);
    static TEST_STATE: std::cell::RefCell<Option<AppState>> = const { std::cell::RefCell::new(None) };
    static RENDER_SCHEDULED: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
}

fn set_test_state(state: AppState) {
    TEST_STATE.with(|s| { *s.borrow_mut() = Some(state); });
}

fn schedule_render() {
    let already = RENDER_SCHEDULED.with(|f| { let was = *f.borrow(); *f.borrow_mut() = true; was });
    if already { return; }

    TEST_STATE.with(|s| {
        let state = s.borrow();
        let Some(state) = state.as_ref() else { return };
        let state_copy = *state;

        let closure = wasm_bindgen::closure::Closure::once(Box::new(move || {
            RENDER_SCHEDULED.with(|f| *f.borrow_mut() = false);
            state_copy.render_version.update(|v| *v += 1);
        }) as Box<dyn FnOnce()>);

        web_sys::window().expect("window")
            .request_animation_frame(closure.as_ref().unchecked_ref()).ok();
        closure.forget();
    });
}

/// Apply full snapshot (640×400×4 = 1,024,000 bytes RGBA)
pub fn apply_snapshot(data: &[u8]) {
    PIXEL_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        let len = data.len().min(buf.len());
        buf[..len].copy_from_slice(&data[..len]);
    });
    schedule_render();
}

/// Apply frame delta: packed [N × 7 bytes: offset3 + rgba]
/// Just memory writes to the pixel buffer — GPU upload happens on next render.
pub fn apply_frame_delta(data: &[u8]) {
    let count = data.len() / 7;
    if count == 0 { return; }

    PIXEL_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        let buf_len = buf.len();
        for i in 0..count {
            let off = i * 7;
            if off + 6 >= data.len() { break; }

            let pixel_offset = ((data[off] as u32) << 16)
                | ((data[off + 1] as u32) << 8)
                | (data[off + 2] as u32);
            let byte_idx = (pixel_offset as usize) * 4;

            if byte_idx + 3 < buf_len {
                buf[byte_idx] = data[off + 3];
                buf[byte_idx + 1] = data[off + 4];
                buf[byte_idx + 2] = data[off + 5];
                buf[byte_idx + 3] = if data[off + 6] != 0 { 0xFF } else { 0x00 };
            }
        }
    });
    schedule_render();
}
