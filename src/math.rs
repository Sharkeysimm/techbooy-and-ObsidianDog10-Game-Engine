use eframe::egui::Vec2;

#[derive(Clone, Copy)]
pub struct Affine {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    tx: f32,
    ty: f32,
}

impl Affine {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    // rotation is counterclockwise positive in degrees like unity
    pub fn from_trs(position: Vec2, rotation_degrees: f32, scale: Vec2) -> Self {
        let (s, c) = (-rotation_degrees.to_radians()).sin_cos();
        Self {
            a: c * scale.x,
            b: s * scale.x,
            c: -s * scale.y,
            d: c * scale.y,
            tx: position.x,
            ty: position.y,
        }
    }

    // applies other first then self
    pub fn mul(&self, o: &Affine) -> Affine {
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }

    pub fn apply(&self, p: Vec2) -> Vec2 {
        Vec2::new(
            self.a * p.x + self.c * p.y + self.tx,
            self.b * p.x + self.d * p.y + self.ty,
        )
    }

    pub fn apply_vector(&self, v: Vec2) -> Vec2 {
        Vec2::new(self.a * v.x + self.c * v.y, self.b * v.x + self.d * v.y)
    }

    pub fn inverse(&self) -> Affine {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-8 {
            return Affine::IDENTITY;
        }

        let a = self.d / det;
        let b = -self.b / det;
        let c = -self.c / det;
        let d = self.a / det;

        Affine {
            a,
            b,
            c,
            d,
            tx: -(a * self.tx + c * self.ty),
            ty: -(b * self.tx + d * self.ty),
        }
    }

    pub fn position(&self) -> Vec2 {
        Vec2::new(self.tx, self.ty)
    }

    // angle in radians in the same space the physics engine uses
    pub fn angle(&self) -> f32 {
        self.b.atan2(self.a)
    }

    pub fn scale(&self) -> Vec2 {
        let det = self.a * self.d - self.b * self.c;
        let sx = self.a.hypot(self.b);
        let sy = self.c.hypot(self.d);
        Vec2::new(sx, if det < 0.0 { -sy } else { sy })
    }

    pub fn decompose(&self) -> (Vec2, f32, Vec2) {
        (self.position(), -self.angle().to_degrees(), self.scale())
    }
}

pub fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

// first vertex points up and the rest go clockwise on screen
pub fn regular_polygon(sides: u32, radius: f32) -> Vec<Vec2> {
    let sides = sides.max(3);

    (0..sides)
        .map(|i| {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 / sides as f32 * std::f32::consts::TAU;
            Vec2::new(angle.cos() * radius, angle.sin() * radius)
        })
        .collect()
}

pub fn capsule_outline(radius: f32, half_segment: f32, steps: usize) -> Vec<Vec2> {
    let mut points = Vec::new();

    for i in 0..=steps {
        let angle = std::f32::consts::PI + i as f32 / steps as f32 * std::f32::consts::PI;
        points.push(Vec2::new(
            angle.cos() * radius,
            angle.sin() * radius - half_segment,
        ));
    }
    for i in 0..=steps {
        let angle = i as f32 / steps as f32 * std::f32::consts::PI;
        points.push(Vec2::new(
            angle.cos() * radius,
            angle.sin() * radius + half_segment,
        ));
    }

    points
}

pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let mut j = polygon.len().wrapping_sub(1);

    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[j]);
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
        j = i;
    }

    inside
}
