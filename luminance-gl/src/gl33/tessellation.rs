use gl;
use gl::types::*;
use gl33::buffer::Buffer;
use gl33::token::GL33;
use luminance::tessellation::{self, HasTessellation, Mode};
use luminance::vertex::{Dim, Type, Vertex, VertexComponentFormat};
use std::mem;
use std::ptr;

pub type Tessellation = tessellation::Tessellation<GL33>;

pub struct GLTess {
  // closure taking the point / line size and the number of instances to render
  pub render: Box<Fn(Option<f32>, u32)>,
  vao: GLenum,
  buffers: Vec<GLenum>
}

impl HasTessellation for GL33 {
  type Tessellation = GLTess;

  fn new<T>(mode: Mode, vertices: &[T], indices: Option<&[u32]>) -> Self::Tessellation where T: Vertex {
    let mut vao: GLuint = 0;
    let vert_nb = vertices.len();

    unsafe {
      gl::GenVertexArrays(1, &mut vao);

      gl::BindVertexArray(vao);

      // vertex buffer
      let vertex_buffer = Buffer::new(vert_nb);
      vertex_buffer.fill(vertices);

      // once the vertex buffer is filled, we get its internal representation’s handle and we leak
      // it so that it’s not dropped at the end of the scope
      let vbo = vertex_buffer.repr.handle;
      mem::forget(vertex_buffer);

      gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
      set_vertex_pointers(&T::vertex_format());

      // in case of indexed render, create the required objects
      if let Some(indices) = indices {
        let ind_nb = indices.len();
        let index_buffer = Buffer::new(ind_nb);
        index_buffer.fill(indices);

        // same than vertex buffer, once the index buffer is filled, we leak it to the void
        let ibo = index_buffer.repr.handle;
        mem::forget(index_buffer);

        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ibo);

        gl::BindVertexArray(0);

        GLTess {
          render: Box::new(move |size, instances| {
            gl::BindVertexArray(vao);

            set_point_line_size(mode, size);

            if instances == 1 {
              gl::DrawElements(opengl_mode(mode), ind_nb as GLsizei, gl::UNSIGNED_INT, ptr::null());
            } else if instances > 1 {
              gl::DrawElementsInstanced(opengl_mode(mode), ind_nb as GLsizei, gl::UNSIGNED_INT, ptr::null(), instances as GLsizei);
            } else {
              panic!("cannot index-render 0 instance");
            }
          }),
          vao: vao,
          buffers: vec![vbo, ibo]
        }
      } else {
        gl::BindVertexArray(0);

        GLTess {
          render: Box::new(move |size, instances| {
            gl::BindVertexArray(vao);

            set_point_line_size(mode, size);

            if instances == 1 {
              gl::DrawArrays(opengl_mode(mode), 0, vert_nb as GLsizei);
            } else if instances > 1 {
              gl::DrawArraysInstanced(opengl_mode(mode), 0, vert_nb as GLsizei, instances as GLsizei);
            } else {
              panic!("cannot render 0 instance");
            }
          }),
          vao: vao,
          buffers: vec![vbo]
        }
      }
    }
  }

  fn destroy(tessellation: &mut Self::Tessellation) {
    // delete vertex array and all bound buffers
    unsafe {
      gl::DeleteVertexArrays(1, &tessellation.vao);

      if !tessellation.buffers.is_empty() {
        gl::DeleteBuffers(tessellation.buffers.len() as GLsizei, tessellation.buffers.as_ptr());
      }
    }
  }

  fn attributeless(mode: Mode, vert_nb: usize) -> Self::Tessellation {
    let mut vao = 0;

    unsafe {
      gl::GenVertexArrays(1, &mut vao);

      gl::BindVertexArray(vao);
      gl::BindVertexArray(0);

      GLTess {
        render: Box::new(move |size, instances| {
          gl::BindVertexArray(vao);

          set_point_line_size(mode, size);

          if instances == 1 {
            gl::DrawArrays(opengl_mode(mode), 0, vert_nb as GLsizei);
          } else if instances > 1 {
            gl::DrawArraysInstanced(opengl_mode(mode), 0, vert_nb as GLsizei, instances as GLsizei);
          } else {
            panic!("cannot render 0 instance");
          }
        }),
        vao: vao,
        buffers: Vec::new(),
      }
    }
  }
}

// Give OpenGL types information on the content of the VBO.
fn set_vertex_pointers(formats: &[VertexComponentFormat]) {
  let vertex_weight = vertex_weight(formats) as GLsizei;
  let mut offset = 0;

  for (i, format) in formats.iter().enumerate() {
    set_component_format(i as u32, vertex_weight, offset, format);
    offset += component_weight(format) as u32;
  }
}

fn set_component_format(i: u32, stride: GLsizei, off: u32, f: &VertexComponentFormat) {
  match f.comp_type {
    Type::Floating => {
      unsafe {
        gl::VertexAttribPointer(i as GLuint, dim_as_size(&f.dim), opengl_sized_type(&f), gl::FALSE, stride, ptr::null().offset(off as isize));
      }
    },
    Type::Integral | Type::Unsigned | Type::Boolean => {
      unsafe {
        gl::VertexAttribIPointer(i as GLuint, dim_as_size(&f.dim), opengl_sized_type(&f), stride, ptr::null().offset(off as isize));
      }
    }
  }

  unsafe {
    gl::EnableVertexAttribArray(i as GLuint);
  }
}

fn dim_as_size(d: &Dim) -> GLint {
  match *d {
    Dim::Dim1 => 1,
    Dim::Dim2 => 2,
    Dim::Dim3 => 3,
    Dim::Dim4 => 4
  }
}

fn opengl_sized_type(f: &VertexComponentFormat) -> GLenum {
  match (f.comp_type, f.comp_size) {
    (Type::Integral, 8) => gl::BYTE,
    (Type::Integral, 16) => gl::SHORT,
    (Type::Integral, 32) => gl::INT,
    (Type::Unsigned, 8) | (Type::Boolean, 8) => gl::UNSIGNED_BYTE,
    (Type::Unsigned, 16) => gl::UNSIGNED_SHORT,
    (Type::Unsigned, 32) => gl::UNSIGNED_INT,
    (Type::Floating, 32) => gl::FLOAT,
    _ => panic!("unsupported vertex component format: {:?}", f)
  }
}

// Weight in bytes of a single vertex.
fn vertex_weight(formats: &[VertexComponentFormat]) -> usize {
  formats.iter().fold(0, |a, f| a + component_weight(f))
}

// Weight in bytes of a vertex component.
fn component_weight(f: &VertexComponentFormat) -> usize {
  dim_as_size(&f.dim) as usize * f.comp_size
}

fn opengl_mode(mode: Mode) -> GLenum {
  match mode {
    Mode::Point => gl::POINTS,
    Mode::Line => gl::LINES,
    Mode::LineStrip => gl::LINE_STRIP,
    Mode::Triangle => gl::TRIANGLES,
    Mode::TriangleFan => gl::TRIANGLE_FAN,
    Mode::TriangleStrip => gl::TRIANGLE_STRIP
  }
}

fn set_point_line_size(mode: Mode, size: Option<f32>) {
  let computed = size.unwrap_or(1.);

  match mode {
    Mode::Point => unsafe { gl::PointSize(computed) },
    Mode::Line | Mode::LineStrip => unsafe { gl::LineWidth(computed) },
    _ => {}
  }
}
