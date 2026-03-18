//! WebGL renderer for checkbox grid
//!
//! Renders 1M+ checkboxes efficiently using GPU:
//! - Checkbox state stored in a texture (1000x1000)
//! - Single full-screen quad rendered
//! - Fragment shader samples texture and applies viewport transform

use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGlProgram, WebGlRenderingContext as GL, WebGlShader, WebGlTexture,
    WebGlUniformLocation,
};

use crate::constants::{COLOR_CHECKED, COLOR_GRID, COLOR_UNCHECKED, GRID_HEIGHT, GRID_WIDTH};

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
    uniform vec2 u_offset;          // Pan offset in pixels
    uniform float u_scale;          // Zoom scale
    uniform float u_cellSize;       // Base cell size in pixels
    uniform vec2 u_gridSize;        // Grid dimensions (1000, 1000)
    uniform vec3 u_colorChecked;
    uniform vec3 u_colorUnchecked;
    uniform vec3 u_colorGrid;
    
    void main() {
        // Convert from normalized coords to pixel coords
        // Flip Y axis: WebGL has Y=0 at bottom, Canvas has Y=0 at top
        vec2 pixelCoord = vec2(v_texCoord.x, 1.0 - v_texCoord.y) * u_resolution;
        
        // Apply viewport transform (inverse of rendering transform)
        vec2 gridPixel = (pixelCoord - u_offset) / (u_cellSize * u_scale);
        
        // Check if we're outside the grid
        if (gridPixel.x < 0.0 || gridPixel.y < 0.0 || 
            gridPixel.x >= u_gridSize.x || gridPixel.y >= u_gridSize.y) {
            gl_FragColor = vec4(u_colorGrid, 1.0);
            return;
        }
        
        // Get cell coordinates
        vec2 cell = floor(gridPixel);
        vec2 cellFrac = fract(gridPixel);
        
        // Calculate gap (1 pixel worth in cell space)
        float gapSize = 1.0 / (u_cellSize * u_scale);
        
        // Draw grid lines (gap between cells)
        if (cellFrac.x < gapSize || cellFrac.y < gapSize) {
            gl_FragColor = vec4(u_colorGrid, 1.0);
            return;
        }
        
        // Sample checkbox state texture
        // Texture is 1000x1000, each pixel stores 8 bits (one byte)
        // We need to unpack the bit for this cell
        float cellIndex = cell.y * u_gridSize.x + cell.x;
        float byteIndex = floor(cellIndex / 8.0);
        float bitIndex = mod(cellIndex, 8.0);
        
        // Calculate texture coordinates for the byte
        // Texture is 125000 bytes = 125000 pixels in a 1D texture mapped to 2D
        // We use a 500x250 texture (125000 pixels)
        float texX = mod(byteIndex, 500.0) / 500.0;
        float texY = floor(byteIndex / 500.0) / 250.0;
        
        vec4 texSample = texture2D(u_checkboxState, vec2(texX, texY));
        
        // Unpack the bit (texSample.r is 0-1, multiply by 255 to get byte value)
        float byteValue = texSample.r * 255.0;
        float bitValue = mod(floor(byteValue / pow(2.0, bitIndex)), 2.0);
        
        // Color based on checked state
        vec3 color = bitValue > 0.5 ? u_colorChecked : u_colorUnchecked;
        gl_FragColor = vec4(color, 1.0);
    }
"#;

pub struct WebGLRenderer {
    gl: GL,
    #[allow(dead_code)]
    program: WebGlProgram,
    state_texture: WebGlTexture,
    // Uniform locations
    u_resolution: WebGlUniformLocation,
    u_offset: WebGlUniformLocation,
    u_scale: WebGlUniformLocation,
    #[allow(dead_code)]
    u_cell_size: WebGlUniformLocation,
    #[allow(dead_code)]
    u_grid_size: WebGlUniformLocation,
    #[allow(dead_code)]
    u_color_checked: WebGlUniformLocation,
    #[allow(dead_code)]
    u_color_unchecked: WebGlUniformLocation,
    #[allow(dead_code)]
    u_color_grid: WebGlUniformLocation,
}

impl WebGLRenderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        // Get WebGL context
        let gl: GL = canvas
            .get_context("webgl")
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

        // Create state texture (500x250 = 125000 pixels for 125000 bytes)
        let state_texture = gl.create_texture().ok_or("Failed to create texture")?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&state_texture));
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);

        // Get uniform locations
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
        let u_color_checked = gl
            .get_uniform_location(&program, "u_colorChecked")
            .ok_or("u_colorChecked not found")?;
        let u_color_unchecked = gl
            .get_uniform_location(&program, "u_colorUnchecked")
            .ok_or("u_colorUnchecked not found")?;
        let u_color_grid = gl
            .get_uniform_location(&program, "u_colorGrid")
            .ok_or("u_colorGrid not found")?;

        // Set static uniforms
        gl.uniform1f(Some(&u_cell_size), crate::constants::CELL_SIZE as f32);
        gl.uniform2f(Some(&u_grid_size), GRID_WIDTH as f32, GRID_HEIGHT as f32);

        // Parse and set colors
        let (cr, cg, cb) = parse_hex_color(COLOR_CHECKED);
        gl.uniform3f(Some(&u_color_checked), cr, cg, cb);

        let (ur, ug, ub) = parse_hex_color(COLOR_UNCHECKED);
        gl.uniform3f(Some(&u_color_unchecked), ur, ug, ub);

        let (gr, gg, gb) = parse_hex_color(COLOR_GRID);
        gl.uniform3f(Some(&u_color_grid), gr, gg, gb);

        Ok(Self {
            gl,
            program,
            state_texture,
            u_resolution,
            u_offset,
            u_scale,
            u_cell_size,
            u_grid_size,
            u_color_checked,
            u_color_unchecked,
            u_color_grid,
        })
    }

    pub fn render(
        &self,
        canvas: &HtmlCanvasElement,
        chunk_data: &[u8],
        offset_x: f64,
        offset_y: f64,
        scale: f64,
    ) {
        let width = canvas.width() as f32;
        let height = canvas.height() as f32;

        self.gl.viewport(0, 0, width as i32, height as i32);

        // Update state texture
        self.gl
            .bind_texture(GL::TEXTURE_2D, Some(&self.state_texture));

        // Convert chunk_data to texture (500x250, LUMINANCE format)
        // Each byte becomes one pixel's luminance value
        let tex_data = chunk_data;
        unsafe {
            let tex_array = js_sys::Uint8Array::view(tex_data);
            self.gl
                .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                    GL::TEXTURE_2D,
                    0,
                    GL::LUMINANCE as i32,
                    500,
                    250,
                    0,
                    GL::LUMINANCE,
                    GL::UNSIGNED_BYTE,
                    Some(&tex_array),
                )
                .expect("Failed to upload texture");
        }

        // Update uniforms
        self.gl.uniform2f(Some(&self.u_resolution), width, height);
        self.gl
            .uniform2f(Some(&self.u_offset), offset_x as f32, offset_y as f32);
        self.gl.uniform1f(Some(&self.u_scale), scale as f32);

        // Draw
        self.gl.draw_arrays(GL::TRIANGLES, 0, 6);
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.gl.viewport(0, 0, width as i32, height as i32);
    }

    /// Immediately render a single cell without texture upload
    /// Used for instant visual feedback on click
    pub fn render_cell_immediate(
        &self,
        canvas: &HtmlCanvasElement,
        col: u32,
        row: u32,
        is_checked: bool,
        offset_x: f64,
        offset_y: f64,
        scale: f64,
    ) {
        let cell_size = crate::constants::CELL_SIZE * scale;

        // Calculate cell position on screen
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
        let scissor_w = (cell_size - 1.0) as i32;
        let scissor_h = (cell_size - 1.0) as i32;

        self.gl.scissor(scissor_x, scissor_y, scissor_w, scissor_h);

        // Clear with the cell color
        let (r, g, b) = if is_checked {
            parse_hex_color(COLOR_CHECKED)
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
