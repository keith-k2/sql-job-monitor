use slint::{ComponentHandle, ModelRc, VecModel};
use sql_job_monitor::{MainWindow, StatusCountRow};

#[test]
fn refresh_controls_sit_on_the_right_of_the_toolbar() {
    let ui = MainWindow::new().expect("create the window");
    ui.set_connected(true);
    ui.set_server_label("warehouse".into());
    ui.set_status_message("Connected".into());
    ui.set_last_refreshed("2026-10-11 12:52:55 AM".into());
    ui.set_status_counts(ModelRc::new(VecModel::from(vec![
        StatusCountRow {
            label: "Running".into(),
            count: 2,
        },
        StatusCountRow {
            label: "Succeeded".into(),
            count: 4,
        },
        StatusCountRow {
            label: "Failed".into(),
            count: 1,
        },
    ])));

    let weak = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
        let ui = weak.upgrade().expect("window still open");
        let row_width = ui.get_toolbar_row_width();
        let actions_right = ui.get_toolbar_actions_right();
        let gap = row_width - actions_right;
        let left_width = ui.get_toolbar_left_width();
        let left_content = ui.get_toolbar_left_content();
        assert!(
            left_content > 200.0 && left_width >= left_content,
            "left group is {left_width}px wide for {left_content}px of server name and status chips"
        );
        assert!(
            row_width > 1000.0,
            "toolbar row is {row_width}px wide, so it is still hugging its contents"
        );
        assert!(
            (gap - 12.0).abs() < 8.0,
            "Disconnect ends {actions_right}px into a {row_width}px row (gap {gap}px, expected 12px)"
        );
        let collapsed_x = ui.get_job_grid_x();
        let collapsed_width = ui.get_job_grid_width();
        ui.set_history_collapsed(false);
        ui.set_history_pane_width(520.0);
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
            let ui = weak.upgrade().expect("window still open");
            let open_x = ui.get_job_grid_x();
            let open_width = ui.get_job_grid_width();
            assert!(
                collapsed_width > 800.0,
                "grid width {collapsed_width}px is not the content area"
            );
            assert_eq!(
                (collapsed_x, collapsed_width),
                (open_x, open_width),
                "opening a 520px history pane moved the grid"
            );
            if let Ok(path) = std::env::var("TOOLBAR_SNAPSHOT") {
                let shot = ui.window().take_snapshot().expect("snapshot");
                write_png(&path, shot.width(), shot.height(), shot.as_bytes());
            }
            slint::quit_event_loop().expect("quit");
        });
    });

    ui.run().expect("run the window");
}

fn write_png(path: &str, width: u32, height: u32, rgba: &[u8]) {
    fn crc(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc(&body).to_be_bytes());
    }

    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in rgba.chunks(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (index, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(index + 1 == blocks.len()));
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    std::fs::write(path, png).expect("write snapshot");
}
