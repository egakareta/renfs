use macroquad::prelude::*;

const SAVE_FILE: &str = "save.txt";
const PANEL: Color = Color::from_rgba(43, 31, 26, 255);
const PANEL_LIGHT: Color = Color::from_rgba(57, 41, 34, 255);
const CREAM: Color = Color::from_rgba(255, 242, 216, 255);
const MUTED: Color = Color::from_rgba(190, 165, 143, 255);
const GOLD: Color = Color::from_rgba(255, 190, 73, 255);

#[derive(Clone, Copy)]
struct Upgrade {
    name: &'static str,
    description: &'static str,
    base_cost: f64,
    growth: f64,
    cookies_per_second: f64,
    click_bonus: f64,
}

const UPGRADES: [Upgrade; 5] = [
    Upgrade {
        name: "Cursor",
        description: "A tiny helper bakes in the background",
        base_cost: 15.0,
        growth: 1.15,
        cookies_per_second: 0.2,
        click_bonus: 0.0,
    },
    Upgrade {
        name: "Grandma",
        description: "A kindly baker and her secret recipe",
        base_cost: 100.0,
        growth: 1.15,
        cookies_per_second: 1.0,
        click_bonus: 0.0,
    },
    Upgrade {
        name: "Bakery",
        description: "Fresh batches, around the clock",
        base_cost: 1_100.0,
        growth: 1.15,
        cookies_per_second: 8.0,
        click_bonus: 0.0,
    },
    Upgrade {
        name: "Cookie factory",
        description: "Industrial-scale deliciousness",
        base_cost: 12_000.0,
        growth: 1.15,
        cookies_per_second: 47.0,
        click_bonus: 0.0,
    },
    Upgrade {
        name: "Stronger clicks",
        description: "Each click bakes one extra cookie",
        base_cost: 50.0,
        growth: 2.0,
        cookies_per_second: 0.0,
        click_bonus: 1.0,
    },
];

#[derive(Default)]
struct GameState {
    cookies: f64,
    total_cookies: f64,
    owned: [u32; 5],
}

impl GameState {
    fn cookies_per_second(&self) -> f64 {
        UPGRADES
            .iter()
            .zip(self.owned)
            .map(|(upgrade, owned)| upgrade.cookies_per_second * f64::from(owned))
            .sum()
    }

    fn click_power(&self) -> f64 {
        1.0 + UPGRADES
            .iter()
            .zip(self.owned)
            .map(|(upgrade, owned)| upgrade.click_bonus * f64::from(owned))
            .sum::<f64>()
    }

    fn cost(&self, index: usize) -> f64 {
        let upgrade = UPGRADES[index];
        upgrade.base_cost * upgrade.growth.powi(self.owned[index] as i32)
    }

    fn save_text(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            self.cookies,
            self.total_cookies,
            self.owned[0],
            self.owned[1],
            self.owned[2],
            self.owned[3],
            self.owned[4],
        )
    }

    fn load_text(text: &str) -> Self {
        let values: Vec<&str> = text.lines().collect();
        let amount = |index: usize| {
            values
                .get(index)
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .unwrap_or(0.0)
        };
        let level = |index: usize| {
            values
                .get(index)
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0)
        };

        Self {
            cookies: amount(0),
            total_cookies: amount(1),
            owned: [level(2), level(3), level(4), level(5), level(6)],
        }
    }
}

struct ClickPopup {
    age: f32,
    amount: f64,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Cookie Workshop".to_owned(),
        window_width: 1100,
        window_height: 720,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let app_dir = renfs::app_dir("cookie_clicker").ok();
    let mut game = app_dir
        .as_ref()
        .and_then(|directory| directory.read_text(SAVE_FILE).ok())
        .map(|text| GameState::load_text(&text))
        .unwrap_or_default();

    let mut cookie_pulse = 0.0_f32;
    let mut popups = Vec::<ClickPopup>::new();
    let mut save_elapsed = 0.0_f32;
    let mut dirty = false;

    loop {
        let dt = get_frame_time().clamp(0.0, 0.25);
        let cps = game.cookies_per_second();
        if cps > 0.0 {
            let earned = cps * f64::from(dt);
            game.cookies += earned;
            game.total_cookies += earned;
            dirty = true;
        }

        cookie_pulse = (cookie_pulse - dt * 3.0).max(0.0);
        for popup in &mut popups {
            popup.age += dt;
        }
        popups.retain(|popup| popup.age < 0.85);

        let width = screen_width();
        let height = screen_height();
        let margin = 22.0;
        let gap = 16.0;
        let header = Rect::new(margin, margin, width - margin * 2.0, 98.0);
        let content_y = header.y + header.h + 14.0;
        let content_h = (height - content_y - margin).max(220.0);
        let content_w = width - margin * 2.0;
        let left_w = (content_w - gap) * 0.56;
        let left = Rect::new(margin, content_y, left_w, content_h);
        let shop = Rect::new(
            left.x + left.w + gap,
            content_y,
            content_w - left_w - gap,
            content_h,
        );

        let cookie_radius = (left.w.min(left.h) * 0.255).clamp(66.0, 132.0);
        let cookie_x = left.x + left.w * 0.5;
        let cookie_y = left.y + left.h * 0.48;
        let hovered_cookie = distance(mouse_position(), (cookie_x, cookie_y)) < cookie_radius * 1.1;

        let card_gap = 9.0;
        let card_top = shop.y + 66.0;
        let card_h = ((shop.h - 79.0 - card_gap * 4.0) / 5.0).clamp(58.0, 92.0);
        let (mouse_x, mouse_y) = mouse_position();
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        let mut state_changed = false;

        if clicked && hovered_cookie {
            let amount = game.click_power();
            game.cookies += amount;
            game.total_cookies += amount;
            cookie_pulse = 1.0;
            popups.push(ClickPopup { age: 0.0, amount });
            state_changed = true;
        } else if clicked {
            for index in 0..UPGRADES.len() {
                let card_y = card_top + index as f32 * (card_h + card_gap);
                if contains(
                    shop.x + 14.0,
                    card_y,
                    shop.w - 28.0,
                    card_h,
                    mouse_x,
                    mouse_y,
                ) {
                    let cost = game.cost(index);
                    if game.cookies >= cost {
                        game.cookies -= cost;
                        game.owned[index] = game.owned[index].saturating_add(1);
                        state_changed = true;
                    }
                    break;
                }
            }
        }

        dirty |= state_changed;
        if dirty {
            save_elapsed += dt;
            if state_changed || save_elapsed >= 1.0 {
                if let Some(directory) = app_dir.as_ref() {
                    let _ = directory.write_file_sync(SAVE_FILE, game.save_text().as_bytes());
                }
                save_elapsed = 0.0;
                dirty = false;
            }
        }

        clear_background(Color::from_rgba(27, 19, 17, 255));
        draw_rectangle(0.0, 0.0, width, height, Color::from_rgba(27, 19, 17, 255));

        draw_panel(header, PANEL);
        draw_text(
            "COOKIE WORKSHOP",
            header.x + 25.0,
            header.y + 39.0,
            27.0,
            CREAM,
        );
        draw_text(
            "A little bakery with big ambitions",
            header.x + 26.0,
            header.y + 67.0,
            16.0,
            MUTED,
        );

        let stat_start = header.x + header.w * 0.43;
        let stat_w = (header.w - (stat_start - header.x) - 16.0) / 3.0;
        draw_header_stat(
            stat_start,
            header.y + 20.0,
            "COOKIES",
            &format_number(game.cookies),
        );
        draw_header_stat(
            stat_start + stat_w,
            header.y + 20.0,
            "PER SECOND",
            &format!("{}/s", format_number(game.cookies_per_second())),
        );
        draw_header_stat(
            stat_start + stat_w * 2.0,
            header.y + 20.0,
            "BAKED TOTAL",
            &format_number(game.total_cookies),
        );

        draw_panel(left, PANEL);
        draw_text(
            "CLICK THE COOKIE",
            left.x + 22.0,
            left.y + 35.0,
            17.0,
            MUTED,
        );
        draw_cookie(
            cookie_x,
            cookie_y,
            cookie_radius,
            hovered_cookie,
            cookie_pulse,
        );
        draw_text_centered(
            "Click to bake",
            cookie_x,
            left.y + left.h - 57.0,
            21.0,
            CREAM,
        );
        draw_text_centered(
            &format!(
                "+{} cookie{} per click",
                format_number(game.click_power()),
                if game.click_power() == 1.0 { "" } else { "s" }
            ),
            cookie_x,
            left.y + left.h - 31.0,
            15.0,
            MUTED,
        );

        for popup in &popups {
            let alpha = (1.0 - popup.age / 0.85).clamp(0.0, 1.0);
            let y = cookie_y - cookie_radius - 12.0 - popup.age * 54.0;
            draw_text(
                &format!("+{}", format_number(popup.amount)),
                cookie_x - 25.0,
                y,
                25.0,
                Color::new(1.0, 0.83, 0.38, alpha),
            );
        }

        draw_panel(shop, PANEL);
        draw_text("BAKERY", shop.x + 20.0, shop.y + 31.0, 21.0, CREAM);
        draw_text(
            "Spend cookies to grow your kitchen",
            shop.x + 20.0,
            shop.y + 52.0,
            14.0,
            MUTED,
        );

        for (index, upgrade) in UPGRADES.iter().enumerate() {
            let card_y = card_top + index as f32 * (card_h + card_gap);
            let card = Rect::new(shop.x + 14.0, card_y, shop.w - 28.0, card_h);
            let cost = game.cost(index);
            let affordable = game.cookies >= cost;
            let hovered = contains(card.x, card.y, card.w, card.h, mouse_x, mouse_y);
            let fill = if affordable {
                if hovered {
                    Color::from_rgba(105, 71, 43, 255)
                } else {
                    PANEL_LIGHT
                }
            } else {
                Color::from_rgba(38, 30, 27, 255)
            };
            draw_panel(card, fill);

            let text_color = if affordable { CREAM } else { MUTED };
            draw_text(upgrade.name, card.x + 14.0, card.y + 25.0, 18.0, text_color);
            draw_text(
                upgrade.description,
                card.x + 14.0,
                card.y + 45.0,
                13.0,
                MUTED,
            );
            let bonus = if upgrade.click_bonus > 0.0 {
                format!("+{} / click", format_number(upgrade.click_bonus))
            } else {
                format!("+{} / sec", format_number(upgrade.cookies_per_second))
            };
            draw_text(&bonus, card.x + 14.0, card.y + card.h - 10.0, 13.0, GOLD);

            let level = format!("Owned: {}", game.owned[index]);
            let level_dims = measure_text(&level, None, 13, 1.0);
            draw_text(
                &level,
                card.x + card.w - level_dims.width - 13.0,
                card.y + 22.0,
                13.0,
                text_color,
            );
            let price = format!("{} cookies", format_number(cost.ceil()));
            let price_dims = measure_text(&price, None, 15, 1.0);
            draw_text(
                &price,
                card.x + card.w - price_dims.width - 13.0,
                card.y + card.h - 10.0,
                15.0,
                if affordable { GOLD } else { MUTED },
            );
        }

        draw_text(
            "Progress is saved automatically with renfs",
            margin,
            height - 7.0,
            13.0,
            Color::from_rgba(145, 121, 104, 255),
        );

        next_frame().await;
    }
}

fn draw_panel(rect: Rect, color: Color) {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        Color::from_rgba(100, 72, 56, 180),
    );
}

fn draw_header_stat(x: f32, y: f32, label: &str, value: &str) {
    draw_text(label, x + 14.0, y + 19.0, 12.0, MUTED);
    draw_text(value, x + 14.0, y + 51.0, 23.0, GOLD);
    draw_line(
        x,
        y + 4.0,
        x,
        y + 62.0,
        1.0,
        Color::from_rgba(100, 72, 56, 180),
    );
}

fn draw_cookie(x: f32, y: f32, radius: f32, hovered: bool, pulse: f32) {
    let scale = 1.0 + pulse * 0.08 + if hovered { 0.025 } else { 0.0 };
    let radius = radius * scale;
    draw_circle(
        x + 4.0,
        y + 10.0,
        radius + 2.0,
        Color::from_rgba(15, 10, 8, 150),
    );
    draw_circle(x, y, radius, Color::from_rgba(153, 83, 38, 255));
    draw_circle(
        x,
        y - 3.0,
        radius * 0.91,
        Color::from_rgba(218, 148, 74, 255),
    );
    draw_circle(
        x - radius * 0.18,
        y - radius * 0.27,
        radius * 0.48,
        Color::from_rgba(235, 174, 96, 90),
    );

    const CHIPS: [(f32, f32); 13] = [
        (-0.48, -0.28),
        (-0.18, -0.57),
        (0.21, -0.48),
        (0.52, -0.27),
        (0.4, 0.08),
        (0.11, 0.24),
        (-0.24, 0.16),
        (-0.55, 0.18),
        (-0.37, 0.52),
        (0.03, 0.55),
        (0.49, 0.46),
        (0.02, -0.06),
        (0.68, 0.08),
    ];
    for (index, (chip_x, chip_y)) in CHIPS.iter().enumerate() {
        let size = if index % 3 == 0 { 0.075 } else { 0.06 };
        let chip_radius = radius * size;
        let cx = x + chip_x * radius;
        let cy = y + chip_y * radius;
        draw_circle(cx, cy, chip_radius, Color::from_rgba(74, 37, 25, 255));
        draw_circle(
            cx - chip_radius * 0.2,
            cy - chip_radius * 0.25,
            chip_radius * 0.28,
            Color::from_rgba(131, 74, 43, 255),
        );
    }

    draw_circle_lines(x, y, radius, 3.0, Color::from_rgba(255, 208, 133, 255));
    if hovered {
        draw_circle_lines(
            x,
            y,
            radius + 8.0,
            2.0,
            Color::from_rgba(255, 210, 130, 120),
        );
    }
}

fn draw_text_centered(text: &str, x: f32, y: f32, size: f32, color: Color) {
    let dimensions = measure_text(text, None, size as u16, 1.0);
    draw_text(text, x - dimensions.width / 2.0, y, size, color);
}

fn contains(x: f32, y: f32, width: f32, height: f32, px: f32, py: f32) -> bool {
    px >= x && px <= x + width && py >= y && py <= y + height
}

fn distance(point: (f32, f32), center: (f32, f32)) -> f32 {
    let dx = point.0 - center.0;
    let dy = point.1 - center.1;
    (dx * dx + dy * dy).sqrt()
}

fn format_number(value: f64) -> String {
    if value >= 1_000_000_000_000.0 {
        format!("{:.2}T", value / 1_000_000_000_000.0)
    } else if value >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value >= 10_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else if value >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}
