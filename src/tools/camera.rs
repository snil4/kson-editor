use std::{default::Default, f32::EPSILON, ops::Sub};

use eframe::{
    egui::{
        epaint::{Mesh, Vertex},
        pos2, vec2, Color32, ComboBox, Pos2, Rect, Sense, Shape, Slider, Stroke, Vec2, Widget,
    },
    epaint::Rgba,
};
use glam::{vec3, Mat4};
use kson::{Chart, Graph, GraphPoint, GraphSectionPoint};

use crate::chart_camera::ChartCamera;

use super::CursorObject;

#[derive(Debug, PartialEq, Clone, Copy)]
enum CameraPaths {
    Zoom,
    RotationX,
}

impl Default for CameraPaths {
    fn default() -> Self {
        Self::Zoom
    }
}

impl ToString for CameraPaths {
    fn to_string(&self) -> String {
        match self {
            CameraPaths::Zoom => "Radius".to_string(),
            CameraPaths::RotationX => "Angle".to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub struct CameraTool {
    radius: f32,
    angle: f32,
    angle_dirty: bool,
    radius_dirty: bool,
    display_line: CameraPaths,
    curving_index: Option<(usize, f64, f64)>,
}

impl CameraTool {
    fn current_graph<'a>(&mut self, chart: &'a kson::Chart) -> &'a Vec<kson::GraphPoint> {
        match self.display_line {
            CameraPaths::Zoom => &chart.camera.cam.body.zoom,
            CameraPaths::RotationX => &chart.camera.cam.body.rotation_x,
        }
    }
}

struct CamreaView {
    desired_size: Vec2,
    camera: ChartCamera,
    meshes: Vec<Mesh>,
}

impl CamreaView {
    const TRACK_LENGH: f32 = 16.0;
    const TRACK_WIDTH: f32 = 1.0;

    pub fn new(desired_size: Vec2, camera: ChartCamera) -> Self {
        Self {
            desired_size,
            camera,
            meshes: Default::default(),
        }
    }

    pub fn add_track(&mut self) {
        let left = -(Self::TRACK_WIDTH / 2.0);
        let right = Self::TRACK_WIDTH / 2.0;

        let mut track_mesh = Mesh::with_texture(Default::default());

        track_mesh.add_colored_rect(
            Rect {
                min: pos2(left, 0.0),
                max: pos2(right, Self::TRACK_LENGH),
            },
            Color32::from_gray(50),
        );

        for i in 0..5 {
            let x = left + (i as f32 + 1.0) * Self::TRACK_WIDTH / 6.0;

            track_mesh.add_colored_rect(
                Rect {
                    min: pos2(x - 0.01, 0.0),
                    max: pos2(x + 0.01, Self::TRACK_LENGH),
                },
                Color32::from_gray(100),
            );
        }

        track_mesh.add_colored_rect(
            Rect {
                min: pos2(left, 0.0),
                max: pos2(left + Self::TRACK_WIDTH / 6.0, Self::TRACK_LENGH),
            },
            Color32::from_rgb(255, 0, 100),
        );

        track_mesh.add_colored_rect(
            Rect {
                min: pos2(right - Self::TRACK_WIDTH / 6.0, 0.0),
                max: pos2(right, Self::TRACK_LENGH),
            },
            Color32::from_rgb(0, 100, 255),
        );

        track_mesh.add_colored_rect(
            Rect {
                min: pos2(left, -0.01),
                max: pos2(right, 0.01),
            },
            Color32::RED,
        );

        self.meshes.push(track_mesh);
    }
    pub fn add_mesh(&mut self, mesh: Mesh) {
        self.meshes.push(mesh)
    }
}

impl Widget for CamreaView {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        let width = ui.available_size_before_wrap().x.max(self.desired_size.x);
        let height = width / (16.0 / 9.0); //16:9 aspect ratio, potentially allow toggle to 9:16

        let (response, painter) = ui.allocate_painter(vec2(width, height), Sense::click());
        let view_rect = response.rect;
        let size = view_rect.size();
        let (projection, camera_transform) = self.camera.matrix(size);
        painter.rect(
            ui.max_rect(),
            0.0,
            Color32::from_rgb(0, 0, 0),
            Stroke::none(),
        );

        for mesh in self.meshes {
            let new_vert_pos = mesh
                .vertices
                .iter()
                .map(|p| vec3(p.pos.x, 0.0, p.pos.y))
                .map(|p| Mat4::from_rotation_y(90_f32.to_radians()).transform_point3(p))
                .map(|p| projection.project_point3(camera_transform.transform_point3(p)))
                .map(|p| p * vec3(1.0, -1.0, 1.0))
                .map(|p| p + vec3(1.0, 1.0, 1.0))
                .map(|p| p / vec3(2.0, 2.0, 2.0))
                .map(|p| pos2(p.x * size.x, p.y * size.y) + view_rect.left_top().to_vec2())
                .collect::<Vec<_>>();

            painter.add(Shape::mesh(Mesh {
                indices: mesh.indices,
                vertices: new_vert_pos
                    .iter()
                    .zip(mesh.vertices)
                    .map(|(new_pos, old_vert)| Vertex {
                        pos: *new_pos,
                        uv: old_vert.uv,
                        color: old_vert.color,
                    })
                    .collect(),
                texture_id: mesh.texture_id,
            }));
        }

        response
    }
}

impl CursorObject for CameraTool {
    fn update(&mut self, _tick: u32, tick_f: f64, lane: f32, _pos: Pos2, chart: &Chart) {
        if let Some((c_idx, _, _)) = self.curving_index {
            let transform_value = |v: f64| (v + 3.0) / 6.0;

            if let Some(section) = self.current_graph(chart).windows(2).nth(c_idx) {
                let a = tick_f - section[0].y as f64;
                let a = a / (section[1].y - section[0].y) as f64;

                //TODO: map b value to match mouse position better
                let point = &section[0];
                let end_point = &section[1];
                let start_value = transform_value(point.vf.unwrap_or(point.v));
                let in_value = lane as f64 / 6.0;
                let value = (in_value - start_value) / (transform_value(end_point.v) - start_value);

                self.curving_index = Some((c_idx, a.max(0.0).min(1.0), value.max(0.0).min(1.0)));
            }
        }
    }

    fn draw(
        &self,
        state: &crate::chart_editor::MainState,
        painter: &eframe::egui::Painter,
    ) -> anyhow::Result<()> {
        let (graph, stroke) = match self.display_line {
            CameraPaths::Zoom => (
                &state.chart.camera.cam.body.zoom,
                Stroke::new(1.0, Rgba::from_rgb(1.0, 1.0, 0.0)),
            ),
            CameraPaths::RotationX => (
                &state.chart.camera.cam.body.rotation_x,
                Stroke::new(1.0, Rgba::from_rgb(0.0, 1.0, 1.0)),
            ),
        };

        state.draw_graph(graph, painter, (-3.0, 3.0), stroke);

        for (i, start_end) in graph.windows(2).enumerate() {
            let (color, points) = if matches!(self.curving_index, Some((ci, _, _)) if ci == i) {
                let new_start = if let Some((_, a, b)) = self.curving_index {
                    GraphPoint {
                        y: start_end[0].y,
                        v: start_end[0].v,
                        vf: start_end[0].vf,
                        a: Some(a),
                        b: Some(b),
                    }
                } else {
                    start_end[0]
                };

                (
                    Rgba::from_rgba_premultiplied(0.0, 1.0, 0.0, 1.0),
                    [new_start, start_end[1]],
                )
            } else {
                (
                    Rgba::from_rgba_premultiplied(0.0, 0.0, 1.0, 1.0),
                    [start_end[0], start_end[1]],
                )
            };

            if let Some(pos) = state
                .screen
                .get_control_point_pos(&points, (-3.0, 3.0), None)
            {
                painter.circle(pos, 5.0, color, Stroke::none());
            }
        }

        if let Some((c_idx, a, b)) = self.curving_index {
            if let Some(points) = graph.windows(2).nth(c_idx) {
                state.draw_graph_segmented(
                    &points
                        .iter()
                        .map(|p| GraphSectionPoint {
                            ry: p.y,
                            v: p.v,
                            vf: p.vf,
                            a: Some(a),
                            b: Some(b),
                        })
                        .collect::<Vec<_>>(),
                    painter,
                    (-3.0, 3.0),
                    Stroke {
                        width: 1.0,
                        color: Color32::GREEN,
                    },
                );
            }
        }

        Ok(())
    }

    fn drag_start(
        &mut self,
        screen: crate::chart_editor::ScreenState,
        _tick: u32,
        _tick_f: f64,
        _lane: f32,
        chart: &kson::Chart,
        _actions: &mut crate::action_stack::ActionStack<kson::Chart>,
        pos: Pos2,
        _modifiers: &crate::Modifiers,
    ) {
        let graph = self.current_graph(chart);

        for (i, points) in graph.windows(2).enumerate() {
            if let Some(control_point) = screen.get_control_point_pos(points, (-3.0, 3.0), None) {
                if control_point.distance(pos) < 5.0 {
                    self.curving_index =
                        Some((i, points[0].a.unwrap_or(0.5), points[0].b.unwrap_or(0.5)));
                }
            }
        }
    }

    fn drag_end(
        &mut self,
        _screen: crate::chart_editor::ScreenState,
        _tick: u32,
        _tick_f: f64,
        _lane: f32,
        _chart: &kson::Chart,
        actions: &mut crate::action_stack::ActionStack<kson::Chart>,
        _pos: Pos2,
    ) {
        if let Some((ci, a, b)) = self.curving_index {
            let new_action = actions.new_action();
            let active_line = self.display_line;
            new_action.action = Box::new(move |chart| {
                let graph = match active_line {
                    CameraPaths::Zoom => &mut chart.camera.cam.body.zoom,
                    CameraPaths::RotationX => &mut chart.camera.cam.body.rotation_x,
                };

                if let Some(point) = graph.get_mut(ci) {
                    point.a = Some(a);
                    point.b = Some(b);
                }

                Ok(())
            });

            new_action.description = format!(
                "Edit curve for camera {}.",
                match self.display_line {
                    CameraPaths::Zoom => "radius",
                    CameraPaths::RotationX => "angle",
                }
            )
        }

        self.curving_index = None
    }

    fn draw_ui(&mut self, state: &mut crate::chart_editor::MainState, ctx: &eframe::egui::Context) {
        //Draw winodw, with a viewport that uses the ChartCamera to project a track in using current camera parameters.
        let cursor_tick = state.get_current_cursor_tick() as f64;

        let old_rad = if self.radius_dirty {
            self.radius
        } else {
            state.chart.camera.cam.body.zoom.value_at(cursor_tick) as f32
        };

        let old_angle = if self.angle_dirty {
            self.angle
        } else {
            state.chart.camera.cam.body.rotation_x.value_at(cursor_tick) as f32
        };

        self.angle = old_angle;
        self.radius = old_rad;

        let camera = ChartCamera {
            center: vec3(0.0, 0.0, 0.0),
            angle: -45.0 - 14.0 * self.angle,
            fov: 70.0,
            radius: (-self.radius + 3.1) / 2.0,
            tilt: 0.0,
            track_length: 16.0,
        };

        eframe::egui::Window::new("Camera")
            .title_bar(true)
            .open(&mut true)
            .resizable(true)
            .show(ctx, |ui| {
                let mut camera_view = CamreaView::new(vec2(300.0, 200.0), camera);
                camera_view.add_track();
                ui.add(camera_view);
                ui.add(Slider::new(&mut self.radius, -3.0..=3.0).text("Radius"));
                ui.add(Slider::new(&mut self.angle, -3.0..=3.0).text("Angle"));

                if old_angle.sub(self.angle).abs() > EPSILON {
                    self.angle_dirty = true;
                }

                if old_rad.sub(self.radius).abs() > EPSILON {
                    self.radius_dirty = true;
                }

                ComboBox::from_label("Display Line")
                    .selected_text(self.display_line.to_string())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.display_line,
                            CameraPaths::Zoom,
                            CameraPaths::Zoom.to_string(),
                        );
                        ui.selectable_value(
                            &mut self.display_line,
                            CameraPaths::RotationX,
                            CameraPaths::RotationX.to_string(),
                        );
                    });

                if ui.button("Add Control Point").clicked() {
                    let new_action = state.actions.new_action();
                    new_action.description = "Added camera control point".to_string();
                    let Self {
                        angle,
                        radius,
                        radius_dirty,
                        angle_dirty,
                        display_line: _,
                        curving_index: _,
                    } = *self;
                    let y = state.cursor_line;
                    new_action.action = Box::new(move |c| {
                        if angle_dirty {
                            c.camera.cam.body.rotation_x.push(kson::GraphPoint {
                                y,
                                v: angle as f64,
                                vf: None,
                                a: Some(0.5),
                                b: Some(0.5),
                            })
                        }
                        if radius_dirty {
                            c.camera.cam.body.zoom.push(kson::GraphPoint {
                                y,
                                v: radius as f64,
                                vf: None,
                                a: Some(0.5),
                                b: Some(0.5),
                            });
                        }

                        //TODO: just insert sorted instead
                        c.camera.cam.body.zoom.sort_by_key(|p| p.y);
                        c.camera.cam.body.rotation_x.sort_by_key(|p| p.y);
                        Ok(())
                    });

                    self.radius_dirty = false;
                    self.angle_dirty = false;
                }
            });
    }
}
