use std::time::Duration;

/// 帧数据结构
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA 格式
    pub timestamp: Duration,
    pub frame_number: u64,
}

impl Frame {
    pub fn new(
        width: u32,
        height: u32,
        data: Vec<u8>,
        timestamp_ms: u64,
        frame_number: u64,
    ) -> Self {
        Self {
            width,
            height,
            data,
            timestamp: Duration::from_millis(timestamp_ms),
            frame_number,
        }
    }

    pub fn pixel_count(&self) -> usize {
        (self.width * self.height) as usize
    }

    pub fn to_rgb(&self) -> Vec<u8> {
        let mut rgb = Vec::with_capacity(self.pixel_count() * 3);
        for chunk in self.data.chunks_exact(4) {
            rgb.push(chunk[0]); // R
            rgb.push(chunk[1]); // G
            rgb.push(chunk[2]); // B
        }
        rgb
    }

    pub fn resize_to(&self, target_width: u32, target_height: u32) -> Frame {
        let img = image::RgbaImage::from_raw(self.width, self.height, self.data.clone())
            .expect("Invalid frame data");
        let resized = image::imageops::resize(
            &img,
            target_width,
            target_height,
            image::imageops::FilterType::Triangle,
        );

        Frame {
            width: target_width,
            height: target_height,
            data: resized.into_raw(),
            timestamp: self.timestamp,
            frame_number: self.frame_number,
        }
    }
}

/// 帧元数据（轻量级，用于传递信息）
#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub timestamp_ms: u64,
    pub frame_number: u64,
}

impl FrameInfo {
    pub fn from_frame(frame: &Frame) -> Self {
        Self {
            width: frame.width,
            height: frame.height,
            timestamp_ms: frame.timestamp.as_millis() as u64,
            frame_number: frame.frame_number,
        }
    }
}

/// 从原生层传递的原始帧数据
#[derive(Debug)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub u_plane: Vec<u8>,
    pub v_plane: Vec<u8>,
    pub timestamp_ms: u64,
    pub frame_number: u64,
}

impl RawFrame {
    pub fn to_rgba(&self) -> Frame {
        let mut rgba_data = vec![0u8; (self.width * self.height * 4) as usize];

        for y in 0..self.height {
            for x in 0..self.width {
                let y_idx = (y * self.width + x) as usize;
                let uv_row = y / 2;
                let uv_col = x / 2;
                let uv_idx = (uv_row * (self.width / 2) + uv_col) as usize;

                let y_val = self.y_plane[y_idx] as f32;
                let u_val = self.u_plane[uv_idx] as f32 - 128.0;
                let v_val = self.v_plane[uv_idx] as f32 - 128.0;

                let r = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
                let g = (y_val - 0.344136 * u_val - 0.714136 * v_val).clamp(0.0, 255.0) as u8;
                let b = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;

                let rgba_idx = y_idx * 4;
                rgba_data[rgba_idx] = r;
                rgba_data[rgba_idx + 1] = g;
                rgba_data[rgba_idx + 2] = b;
                rgba_data[rgba_idx + 3] = 255;
            }
        }

        Frame::new(
            self.width,
            self.height,
            rgba_data,
            self.timestamp_ms,
            self.frame_number,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_creation() {
        let data = vec![255u8; 100 * 100 * 4]; // 100x100 white image
        let frame = Frame::new(100, 100, data.clone(), 1000, 30);

        assert_eq!(frame.width, 100);
        assert_eq!(frame.height, 100);
        assert_eq!(frame.pixel_count(), 10000);
        assert_eq!(frame.timestamp.as_millis(), 1000);
        assert_eq!(frame.frame_number, 30);
    }

    #[test]
    fn test_frame_resize() {
        let data = vec![255u8; 100 * 100 * 4];
        let frame = Frame::new(100, 100, data, 0, 0);
        let resized = frame.resize_to(32, 32);

        assert_eq!(resized.width, 32);
        assert_eq!(resized.height, 32);
        assert_eq!(resized.data.len(), 32 * 32 * 4);
    }

    #[test]
    fn test_yuv_to_rgba() {
        let width = 64;
        let height = 64;
        let y_plane = vec![128u8; (width * height) as usize];
        let u_plane = vec![128u8; (width * height / 4) as usize];
        let v_plane = vec![128u8; (width * height / 4) as usize];

        let raw_frame = RawFrame {
            width,
            height,
            y_plane,
            u_plane,
            v_plane,
            timestamp_ms: 0,
            frame_number: 0,
        };

        let frame = raw_frame.to_rgba();
        assert_eq!(frame.width, width);
        assert_eq!(frame.height, height);
        assert_eq!(frame.data.len(), (width * height * 4) as usize);
    }
}
