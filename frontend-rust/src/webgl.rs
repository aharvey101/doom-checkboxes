//! WebGL renderer for checkbox grid
//!
//! Renders 1M+ checkboxes efficiently using GPU:
//! - Checkbox state stored in a texture (1000x1000)
//! - Single full-screen quad rendered
//! - Fragment shader samples texture and applies viewport transform

use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGlBuffer, WebGlProgram, WebGlRenderingContext as GL, WebGlShader,
    WebGlTexture, WebGlUniformLocation,
};

use crate::constants::{CELL_SIZE, CHUNK_SIZE, COLOR_GRID, COLOR_UNCHECKED};
use crate::utils::visible_chunk_range;
use std::collections::HashMap;

const VERTEX_SHADER: &str = r#"
    attribute vec2 a_position;
    varying vec2 v_texCoord;
    
    void main() {
        gl_Position = vec4(a_position, 0.0, 1.0);
        // Convert from clip space (-1 to 1) to texture coords (0 to 1)
        v_texCoord = (a_position + 1.0) / 2.0;
    }
"#;

const FRAGMENT_SHADER: &str = r#"
    precision mediump float;
    
    varying vec2 v_texCoord;
    
    uniform sampler2D u_checkboxState;
    uniform vec2 u_resolution;      // Canvas size in pixels
    uniform vec2 u_offset;          // Chunk offset in pixels (where chunk starts on screen)
    uniform float u_scale;          // Zoom scale
    uniform float u_cellSize;       // Base cell size in pixels
    uniform vec2 u_gridSize;        // Grid dimensions (1000, 1000)
    uniform vec3 u_colorUnchecked;
    uniform vec3 u_colorGrid;
    
    void main() {
        // Convert from normalized coords to pixel coords
        // Flip Y axis: WebGL has Y=0 at bottom, Canvas has Y=0 at top
        vec2 pixelCoord = vec2(v_texCoord.x, 1.0 - v_texCoord.y) * u_resolution;
        
        // Calculate position within the chunk (relative to chunk origin)
        vec2 gridPixel = (pixelCoord - u_offset) / (u_cellSize * u_scale);
        
        // Check if we're outside this chunk's grid
        if (gridPixel.x < 0.0 || gridPixel.y < 0.0 || 
            gridPixel.x >= u_gridSize.x || gridPixel.y >= u_gridSize.y) {
            gl_FragColor = vec4(u_colorGrid, 1.0);
            return;
        }
        
        // Get cell coordinates within chunk
        vec2 cell = floor(gridPixel);
        vec2 cellFrac = fract(gridPixel);
        
        // Calculate gap (1 pixel worth in cell space)
        // Cap at 0.15 so grid lines don't consume entire cell at low zoom
        float gapSize = min(1.0 / (u_cellSize * u_scale), 0.15);
        
        // Draw grid lines (gap between cells)
        if (cellFrac.x < gapSize || cellFrac.y < gapSize) {
            gl_FragColor = vec4(u_colorGrid, 1.0);
            return;
        }
        
        // Sample checkbox state texture directly
        // Texture is 1000x1000 RGBA, each pixel stores [R, G, B, checked]
        // Add 0.5 to sample center of texel
        vec2 texCoord = (cell + 0.5) / u_gridSize;
        vec4 texSample = texture2D(u_checkboxState, texCoord);
        
        // Alpha channel contains checked state (0.0 = unchecked, 1.0 = checked)
        // RGB channels contain the user's color
        if (texSample.a > 0.5) {
            // Checked - use the color stored in the texture
            gl_FragColor = vec4(texSample.rgb, 1.0);
        } else {
            // Unchecked - use default unchecked color
            gl_FragColor = vec4(u_colorUnchecked, 1.0);
        }
    }
"#;

pub struct WebGLRenderer {
    gl: GL,
    program: WebGlProgram,
    vertex_buffer: web_sys::WebGlBuffer,
    a_position: u32,
    state_texture: WebGlTexture,
    // Uniform locations
    u_checkbox_state: WebGlUniformLocation,
    u_resolution: WebGlUniformLocation,
    u_offset: WebGlUniformLocation,
    u_scale: WebGlUniformLocation,
    #[allow(dead_code)]
    u_cell_size: WebGlUniformLocation,
    #[allow(dead_code)]
    u_grid_size: WebGlUniformLocation,
    #[allow(dead_code)]
    u_color_unchecked: WebGlUniformLocation,
    #[allow(dead_code)]
    u_color_grid: WebGlUniformLocation,
}

impl WebGLRenderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        // Get WebGL context with preserveDrawingBuffer to allow incremental rendering
        let context_options = js_sys::Object::new();
        js_sys::Reflect::set(
            &context_options,
            &"preserveDrawingBuffer".into(),
            &true.into(),
        )
        .unwrap();

        let gl: GL = canvas
            .get_context_with_context_options("webgl", &context_options)
            .map_err(|e| format!("Failed to get WebGL context: {:?}", e))?
            .ok_or("WebGL not supported")?
            .dyn_into()
            .map_err(|_| "Failed to cast to WebGlRenderingContext")?;

        // Compile shaders
        let vert_shader = compile_shader(&gl, GL::VERTEX_SHADER, VERTEX_SHADER)?;
        let frag_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, FRAGMENT_SHADER)?;

        // Link program
        let program = link_program(&gl, &vert_shader, &frag_shader)?;
        gl.use_program(Some(&program));

        // Create full-screen quad vertices
        let vertices: [f32; 12] = [
            -1.0, -1.0, // bottom-left
            1.0, -1.0, // bottom-right
            -1.0, 1.0, // top-left
            -1.0, 1.0, // top-left
            1.0, -1.0, // bottom-right
            1.0, 1.0, // top-right
        ];

        let buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer));

        // Upload vertex data
        unsafe {
            let vert_array = js_sys::Float32Array::view(&vertices);
            gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &vert_array, GL::STATIC_DRAW);
        }

        // Set up vertex attribute
        let a_position = gl.get_attrib_location(&program, "a_position") as u32;
        gl.enable_vertex_attrib_array(a_position);
        gl.vertex_attrib_pointer_with_i32(a_position, 2, GL::FLOAT, false, 0, 0);

        // Create state texture (1000x1000 RGBA for 1M cells with RGB + checked state)
        let state_texture = gl.create_texture().ok_or("Failed to create texture")?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&state_texture));
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);

        // Get uniform locations
        let u_checkbox_state = gl
            .get_uniform_location(&program, "u_checkboxState")
            .ok_or("u_checkboxState not found")?;
        let u_resolution = gl
            .get_uniform_location(&program, "u_resolution")
            .ok_or("u_resolution not found")?;
        let u_offset = gl
            .get_uniform_location(&program, "u_offset")
            .ok_or("u_offset not found")?;
        let u_scale = gl
            .get_uniform_location(&program, "u_scale")
            .ok_or("u_scale not found")?;
        let u_cell_size = gl
            .get_uniform_location(&program, "u_cellSize")
            .ok_or("u_cellSize not found")?;
        let u_grid_size = gl
            .get_uniform_location(&program, "u_gridSize")
            .ok_or("u_gridSize not found")?;
        let u_color_unchecked = gl
            .get_uniform_location(&program, "u_colorUnchecked")
            .ok_or("u_colorUnchecked not found")?;
        let u_color_grid = gl
            .get_uniform_location(&program, "u_colorGrid")
            .ok_or("u_colorGrid not found")?;

        // Set static uniforms
        gl.uniform1f(Some(&u_cell_size), crate::constants::CELL_SIZE as f32);
        // Each chunk is CHUNK_SIZE x CHUNK_SIZE, not the full grid
        gl.uniform2f(Some(&u_grid_size), CHUNK_SIZE as f32, CHUNK_SIZE as f32);
        // Set sampler to texture unit 0
        gl.uniform1i(Some(&u_checkbox_state), 0);

        // Parse and set colors (only unchecked and grid colors needed now)
        let (ur, ug, ub) = parse_hex_color(COLOR_UNCHECKED);
        gl.uniform3f(Some(&u_color_unchecked), ur, ug, ub);

        let (gr, gg, gb) = parse_hex_color(COLOR_GRID);
        gl.uniform3f(Some(&u_color_grid), gr, gg, gb);

        Ok(Self {
            gl,
            program,
            vertex_buffer: buffer,
            a_position,
            state_texture,
            u_checkbox_state,
            u_resolution,
            u_offset,
            u_scale,
            u_cell_size,
            u_grid_size,
            u_color_unchecked,
            u_color_grid,
        })
    }

    pub fn render(
        &self,
        canvas: &HtmlCanvasElement,
        loaded_chunks: &HashMap<i64, Vec<u8>>,
        offset_x: f64,
        offset_y: f64,
        scale: f64,
    ) {
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        // Ensure program is active for all drawing
        self.gl.use_program(Some(&self.program));

        // Re-bind vertex buffer and attribute (may have been unbound by other GL operations)
        self.gl
            .bind_buffer(GL::ARRAY_BUFFER, Some(&self.vertex_buffer));
        self.gl.enable_vertex_attrib_array(self.a_position);
        self.gl
            .vertex_attrib_pointer_with_i32(self.a_position, 2, GL::FLOAT, false, 0, 0);

        self.gl.viewport(0, 0, width as i32, height as i32);

        // Clear with grid background
        let (bg_r, bg_g, bg_b) = parse_hex_color(COLOR_GRID);
        self.gl.clear_color(bg_r, bg_g, bg_b, 1.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT);

        // Calculate visible chunk range (signed coordinates)
        let (min_cx, min_cy, max_cx, max_cy) =
            visible_chunk_range(offset_x, offset_y, scale, width, height);

        // Create empty chunk data for unloaded chunks (lazily, reused)
        let empty_chunk: Vec<u8> = vec![0u8; crate::constants::CHUNK_DATA_SIZE];

        // Render each visible chunk (loaded or empty)
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                // Use chunk coordinates to find chunk in loaded_chunks
                let chunk_id = crate::utils::chunk_coords_to_id(cx, cy);
                let chunk_data = loaded_chunks.get(&chunk_id).unwrap_or(&empty_chunk);
                self.render_chunk(canvas, cx, cy, chunk_data, offset_x, offset_y, scale);
            }
        }
    }

    fn render_chunk(
        &self,
        canvas: &HtmlCanvasElement,
        chunk_x: i32,
        chunk_y: i32,
        chunk_data: &[u8],
        offset_x: f64,
        offset_y: f64,
        scale: f64,
    ) {
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        // Calculate chunk's world position using signed coordinates
        let chunk_pixel_size = CHUNK_SIZE as f64 * CELL_SIZE * scale;
        let chunk_screen_x = offset_x + (chunk_x as f64) * chunk_pixel_size;
        let chunk_screen_y = offset_y + (chunk_y as f64) * chunk_pixel_size;

        // Scissor test to only draw within this chunk's region
        self.gl.enable(GL::SCISSOR_TEST);

        // Calculate the visible portion of this chunk on screen
        // In Canvas2D coords (Y=0 at top), the chunk occupies:
        //   X: [chunk_screen_x, chunk_screen_x + chunk_pixel_size]
        //   Y: [chunk_screen_y, chunk_screen_y + chunk_pixel_size]

        // Clamp to canvas bounds
        let visible_left = chunk_screen_x.max(0.0);
        let visible_right = (chunk_screen_x + chunk_pixel_size).min(width);
        let visible_top = chunk_screen_y.max(0.0);
        let visible_bottom = (chunk_screen_y + chunk_pixel_size).min(height);

        // Calculate scissor dimensions
        let scissor_w = (visible_right - visible_left) as i32;
        let scissor_h = (visible_bottom - visible_top) as i32;

        // Convert to WebGL coordinates (Y=0 at bottom)
        // scissor_x is the left edge in WebGL coords (same as Canvas2D)
        let scissor_x = visible_left as i32;
        // scissor_y is the BOTTOM edge in WebGL coords
        // Canvas2D visible_bottom corresponds to WebGL (height - visible_bottom)
        let scissor_y = (height - visible_bottom) as i32;

        if scissor_w <= 0 || scissor_h <= 0 {
            self.gl.disable(GL::SCISSOR_TEST);
            return;
        }

        self.gl.scissor(scissor_x, scissor_y, scissor_w, scissor_h);

        // Upload chunk texture
        self.gl.active_texture(GL::TEXTURE0);
        self.gl
            .bind_texture(GL::TEXTURE_2D, Some(&self.state_texture));

        // Convert chunk_data to texture (1000x1000, RGBA format)
        // Each cell is 4 bytes: [R, G, B, checked]
        // RGBA texture maps directly to this format
        unsafe {
            let tex_array = js_sys::Uint8Array::view(chunk_data);
            self.gl
                .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                    GL::TEXTURE_2D,
                    0,
                    GL::RGBA as i32,
                    CHUNK_SIZE as i32,  // 1000
                    CHUNK_SIZE as i32,  // 1000
                    0,
                    GL::RGBA,
                    GL::UNSIGNED_BYTE,
                    Some(&tex_array),
                )
                .expect("Failed to upload texture");
        }

        // Update uniforms for this chunk's position
        self.gl
            .uniform2f(Some(&self.u_resolution), width as f32, height as f32);
        self.gl.uniform2f(
            Some(&self.u_offset),
            chunk_screen_x as f32,
            chunk_screen_y as f32,
        );
        self.gl.uniform1f(Some(&self.u_scale), scale as f32);

        // Draw
        self.gl.draw_arrays(GL::TRIANGLES, 0, 6);

        self.gl.disable(GL::SCISSOR_TEST);
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.gl.viewport(0, 0, width as i32, height as i32);
    }

    /// Immediately render a single cell without texture upload
    /// Used for instant visual feedback on click
    pub fn render_cell_immediate(
        &self,
        canvas: &HtmlCanvasElement,
        col: i32,
        row: i32,
        is_checked: bool,
        color: (u8, u8, u8), // RGB color for checked state
        offset_x: f64,
        offset_y: f64,
        scale: f64,
    ) {
        let cell_size = CELL_SIZE * scale;

        // Calculate cell position on screen (global coordinates)
        let x = offset_x + (col as f64) * cell_size;
        let y = offset_y + (row as f64) * cell_size;

        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        // Skip if outside visible area
        if x + cell_size < 0.0 || x > width || y + cell_size < 0.0 || y > height {
            return;
        }

        // Use scissor test to only draw in the cell area
        self.gl.enable(GL::SCISSOR_TEST);

        // WebGL scissor Y is from bottom, need to flip
        let scissor_x = (x + 0.5) as i32;
        let scissor_y = (height - y - cell_size + 0.5) as i32;
        let scissor_w = (cell_size - 1.0).max(1.0) as i32;
        let scissor_h = (cell_size - 1.0).max(1.0) as i32;

        self.gl.scissor(scissor_x, scissor_y, scissor_w, scissor_h);

        // Clear with the cell color
        let (r, g, b) = if is_checked {
            // Use the user's color for checked state
            (
                color.0 as f32 / 255.0,
                color.1 as f32 / 255.0,
                color.2 as f32 / 255.0,
            )
        } else {
            parse_hex_color(COLOR_UNCHECKED)
        };

        self.gl.clear_color(r, g, b, 1.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT);

        self.gl.disable(GL::SCISSOR_TEST);
    }
}

fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or("Failed to create shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let log = gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| "Unknown error".to_string());
        Err(format!("Shader compilation failed: {}", log))
    }
}

fn link_program(gl: &GL, vert: &WebGlShader, frag: &WebGlShader) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("Failed to create program")?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        let log = gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| "Unknown error".to_string());
        Err(format!("Program linking failed: {}", log))
    }
}

fn parse_hex_color(hex: &str) -> (f32, f32, f32) {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
    (r, g, b)
}
