use cgmath::InnerSpace;
use wgpu::util::DeviceExt;
use winit::event::{DeviceEvent, ElementState, KeyboardInput, VirtualKeyCode};
use std::f32::consts::FRAC_PI_2;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

#[derive(Debug)]
pub struct CameraData {
    pub position: cgmath::Point3<f32>,
    yaw: cgmath::Rad<f32>,
    pitch: cgmath::Rad<f32>,
}

impl CameraData {

    pub fn new<V: Into<cgmath::Point3<f32>>, Y: Into<cgmath::Rad<f32>>, P: Into<cgmath::Rad<f32>>>(position: V, yaw: Y, pitch: P) -> Self {
        Self {
            position: position.into(),
            yaw: yaw.into(),
            pitch: pitch.into()
        }
    }

    fn calc_matrix(&self) -> cgmath::Matrix4<f32> {
        cgmath::Matrix4::look_to_rh(
            self.position,
            cgmath::Vector3::new(
                self.yaw.0.cos(),
                self.pitch.0.sin(),
                self.yaw.0.sin(),
            ).normalize(),
            cgmath::Vector3::unit_y()
        )
    }
}

pub struct Projection {
    aspect: f32,
    fovy: cgmath::Rad<f32>,
    znear: f32,
    zfar: f32
}

impl Projection {

    pub fn new<F: Into<cgmath::Rad<f32>>>(width: u32, height: u32, fovy: F, znear: f32, zfar: f32) -> Self {

        Self {
            aspect: width as f32 / height as f32,
            fovy: fovy.into(),
            znear,
            zfar
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    fn calc_matrix(&self) -> cgmath::Matrix4<f32> {
        OPENGL_TO_WGPU_MATRIX * cgmath::perspective(self.fovy, self.aspect, self.znear, self.zfar)
    }
}

#[derive(Debug)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
    speed: f32,
    sensitivity: f32,
}

impl CameraController {

    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
            speed,
            sensitivity
        }
    }

    fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {

        let amount = if state == ElementState::Pressed { 1.0 } else { 0.0 };
        match key {
            VirtualKeyCode::W | VirtualKeyCode::Up => {
                self.amount_forward = amount;
                true
            }
            VirtualKeyCode::S | VirtualKeyCode::Down => {
                self.amount_backward = amount;
                true
            }
            VirtualKeyCode::A | VirtualKeyCode::Left => {
                self.amount_left = amount;
                true
            }
            VirtualKeyCode::D | VirtualKeyCode::Right => {
                self.amount_right = amount;
                true
            }
            VirtualKeyCode::Space => {
                self.amount_up = amount;
                true
            }
            VirtualKeyCode::LShift => {
                self.amount_down = amount;
                true
            }
            _ => false
        }
    }

    fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    fn process_scroll(&mut self, delta: &winit::event::MouseScrollDelta) {

        self.scroll = -match delta {
            winit::event::MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            winit::event::MouseScrollDelta::PixelDelta(
                winit::dpi::PhysicalPosition {
                    y: scroll,
                    ..
                }
            ) => *scroll as f32
        };
    }

    fn update_camera(&mut self, camera: &mut CameraData, dt: std::time::Duration) {

        let dt = dt.as_secs_f32();

        // forward/backward
        let (yaw_sin, yaw_cos) = camera.yaw.0.sin_cos();
        let forward = cgmath::Vector3::new(yaw_cos, 0.0, yaw_sin).normalize();
        let right = cgmath::Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();

        camera.position += forward * (self.amount_forward - self.amount_backward) * self.speed * dt;
        camera.position += right * (self.amount_right - self.amount_left) * self.speed * dt;

        // move in/out where we are looking (like a zoom, but altering the camera's position)
        let (pitch_sin, pitch_cos) = camera.pitch.0.sin_cos();
        let scrollward = cgmath::Vector3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();
        camera.position += scrollward * self.scroll * self.speed * self.sensitivity * dt;
        self.scroll = 0.0;

        // Move up/down. Since we don't use roll, we can just
        // modify the y coordinate directly.
        camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        // Rotate
        camera.yaw += cgmath::Rad(self.rotate_horizontal) * self.sensitivity * dt;
        camera.pitch += cgmath::Rad(-self.rotate_vertical) * self.sensitivity * dt;

        // If process_mouse isn't called every frame, these values
        // will not get set to zero, and the camera will rotate
        // when moving in a non cardinal direction.
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        // Keep the camera's angle from going too high/low.
        if camera.pitch < -cgmath::Rad(FRAC_PI_2) {
            camera.pitch = -cgmath::Rad(FRAC_PI_2);
        } else if camera.pitch > cgmath::Rad(FRAC_PI_2) {
            camera.pitch = cgmath::Rad(FRAC_PI_2);
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {

    // can't use cgmath with bytemuck directly
    view_proj: [[f32; 4]; 4]
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }
    pub fn update_view_proj(&mut self, camera: &CameraData, projection: &Projection) {
        self.view_proj = (projection.calc_matrix() * camera.calc_matrix()).into();
    }
}

pub struct Camera {

    data: CameraData,
    projection: Projection,
    controller: CameraController,
    uniform: CameraUniform,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mouse_pressed: bool
        
}

impl Camera {

    pub fn new(device: &wgpu::Device, data: CameraData, projection: Projection, controller: CameraController) -> (Self, wgpu::BindGroupLayout) {

        let mut uniform = CameraUniform::new();
        uniform.update_view_proj(&data, &projection);

        let buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
            }
        );

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    count: None
                }
            ],
            label: Some("camera_bind_group_layout")
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {

            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding()
                }
            ],
            label: Some("camera_bind_group")
        });

        (
            Self {
                data,
                projection,
                controller,
                uniform,
                buffer,
                bind_group,
                mouse_pressed: false
            },
            camera_bind_group_layout
        )

    }

    pub fn get_bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn resize_projection(&mut self, new_size: &winit::dpi::PhysicalSize<u32>) {
        self.projection.resize(new_size.width, new_size.height);
    }

    pub fn process_input(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::Key(
                KeyboardInput {
                    virtual_keycode: Some(key),
                    state,
                    ..
                }
            ) => self.controller.process_keyboard(*key, *state),
            DeviceEvent::MouseWheel { delta, .. } => {
                self.controller.process_scroll(&delta);
                true
            }
            DeviceEvent::Button {
                button: 1,
                state,
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            DeviceEvent::MouseMotion { delta } => {
                if self.mouse_pressed {
                    self.controller.process_mouse(delta.0, delta.1);
                }
                true
            }
            _ => false
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: std::time::Duration) {

        self.controller.update_camera(&mut self.data, dt);
        self.uniform.update_view_proj(&self.data, &self.projection);
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}
