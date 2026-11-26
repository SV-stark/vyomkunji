// Screen rendering: bars, views, panels and diagnostics.
use super::*;

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Closing the window keeps the app alive in the tray when close_to_tray is enabled.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quit_requested
            && self.close_to_tray
        {
            self.hide_intent = true;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.pump_requests();
        self.drain_tray();

        // Idle auto-lock: clear the in-memory PIN after the configured timeout.
        if !self.pin.is_empty() {
            let expired = self
                .pin_set_at
                .map(|t| t.elapsed() >= self.pin_timeout)
                .unwrap_or(false);
            if expired {
                self.pin = Zeroizing::new(String::new());
                self.pin_set_at = None;
                self.set_status(Level::Info, "PIN cleared after idle timeout".to_string());
            }
        }

        // TEST MODE: auto-connect and auto-approve when VYOMKUNJI_GUI_TEST_PIN (or SIGNER_GUI_TEST_PIN) is set.
        if let Ok(pin) = std::env::var("VYOMKUNJI_GUI_TEST_PIN")
            .or_else(|_| std::env::var("SIGNER_GUI_TEST_PIN"))
        {
            if self.manager.is_none() {
                self.connect();
            }
            self.pin = Zeroizing::new(pin);
            if self.pending.is_some() {
                self.approve_pending();
            }
        }

        egui::Panel::top("top")
            .exact_size(60.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::surface())
                    .stroke(egui::Stroke::new(1.0_f32, border()))
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::left("rail")
            .exact_size(238.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::surface())
                    .stroke(egui::Stroke::new(0.0_f32, border()))
                    .inner_margin(egui::Margin::symmetric(12, 16)),
            )
            .show(ui, |ui| self.left_rail(ui));

        egui::Panel::bottom("status")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::surface())
                    .stroke(egui::Stroke::new(1.0_f32, border()))
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ui, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::bg())
                    .inner_margin(egui::Margin::same(24)),
            )
            .show(ui, |ui| match self.nav {
                Nav::Requests => self.view_requests(ui),
                Nav::Certificates => self.view_certificates(ui),
                Nav::Sign => self.view_sign(ui),
                Nav::Portals => self.view_portals(ui),
                Nav::Activity => self.view_activity(ui),
                Nav::Settings => self.view_settings(ui),
            });

        let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc_pressed && self.pending.is_some() {
            self.cancel();
        }
    }
}

impl App {
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 10.0, accent());
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "V",
                Weight::Bold.font_id(19.0),
                egui::Color32::WHITE,
            );

            ui.vertical(|ui| {
                ui.add_space(1.0);
                ui.label(
                    egui::RichText::new("VyomKunji")
                        .font(Weight::SemiBold.font_id(17.0))
                        .color(text()),
                );
                ui.label(
                    egui::RichText::new(concat!(
                        "India G2C · DSC signing · v",
                        env!("CARGO_PKG_VERSION")
                    ))
                    .font(Weight::Regular.font_id(11.0))
                    .color(text_faint()),
                );
            });

            ui.add_space(6.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Quit").clicked() {
                    self.quit_requested = true;
                    if let Some(ctx) = &self.ctx {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                if ui.button("Refresh token").clicked() {
                    self.connect();
                }
                let is_dark = theme::active_base() == fastframe_theme::Base::Dark;
                if ui
                    .button(if is_dark { "Light mode" } else { "Dark mode" })
                    .clicked()
                {
                    let next_dark = !is_dark;
                    theme::set_base(if next_dark {
                        fastframe_theme::Base::Dark
                    } else {
                        fastframe_theme::Base::Light
                    });
                    let _ = pkcs11::save_config_key("darkMode", serde_json::json!(next_dark));
                    let ctx = ui.ctx().clone();
                    self.apply_theme(&ctx);
                }
                let (dot_col, label) = match (self.demo_key, &self.manager, self.pending.is_some())
                {
                    (_, _, true) => (warn(), "Awaiting approval"),
                    (true, _, false) => (warn(), "Demo key"),
                    (false, Some(_), false) => (ok(), "Ready"),
                    (false, None, false) => (err(), "No token"),
                };
                let font = Weight::Medium.font_id(11.5);
                let galley = ui
                    .painter()
                    .layout_no_wrap(label.to_string(), font.clone(), dot_col);
                let size = galley.size() + egui::vec2(34.0, 10.0);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, theme::radius_sm(), theme::tint(dot_col, 34));
                ui.painter().circle_filled(
                    rect.left_center() + egui::vec2(12.0, 0.0),
                    4.0,
                    dot_col,
                );
                ui.painter().text(
                    rect.left_center() + egui::vec2(22.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font,
                    dot_col,
                );
            });
        });
    }

    fn left_rail(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("WORKSPACE")
                .font(Weight::SemiBold.font_id(10.0))
                .color(text_faint()),
        );
        ui.add_space(6.0);

        let items = [
            (Nav::Requests, self.pending.is_some()),
            (Nav::Certificates, false),
            (Nav::Sign, false),
            (Nav::Portals, false),
            (Nav::Activity, false),
            (Nav::Settings, false),
        ];
        for (nav, badge) in items {
            let selected = self.nav == nav;
            let (rect, resp) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
            if selected {
                ui.painter()
                    .rect_filled(rect, theme::radius_sm(), theme::tint(accent(), 40));
                let bar = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(0.0, 9.0),
                    egui::vec2(3.0, 20.0),
                );
                ui.painter().rect_filled(bar, 2.0, accent());
            } else if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, theme::radius_sm(), theme::tint(text_dim(), 22));
            }
            let col = if selected { text() } else { text_dim() };
            ui.painter().text(
                rect.left_center() + egui::vec2(14.0, 0.0),
                egui::Align2::LEFT_CENTER,
                nav.title(),
                Weight::Medium.font_id(13.0),
                col,
            );
            if badge {
                ui.painter().circle_filled(
                    rect.right_center() + egui::vec2(-14.0, 0.0),
                    4.0,
                    warn(),
                );
            }
            if resp.clicked() {
                self.nav = nav;
            }
        }

        ui.add_space(18.0);
        ui.label(
            egui::RichText::new("TOKEN")
                .font(Weight::SemiBold.font_id(10.0))
                .color(text_faint()),
        );
        ui.add_space(6.0);

        let (label, serial, connected) = match self.certs.first() {
            Some(c) => (
                c.token_label.clone(),
                c.token_serial.clone(),
                self.manager.is_some(),
            ),
            None => ("—".to_string(), "—".to_string(), false),
        };
        theme::card_frame(Some(theme::tint(text_dim(), 14))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let col = if connected { ok() } else { err() };
                ui.painter().circle_filled(
                    ui.min_rect().left_center() + egui::vec2(6.0, 0.0),
                    4.0,
                    col,
                );
                ui.label(egui::RichText::new(&label).size(12.5).color(text()));
            });
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(&serial)
                    .family(egui::FontFamily::Monospace)
                    .size(10.5)
                    .color(text_faint()),
            );
        });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            if let Some(p) = self.bridge_port {
                ui.label(
                    egui::RichText::new(format!("bridge 127.0.0.1:{}", p))
                        .family(egui::FontFamily::Monospace)
                        .size(10.0)
                        .color(text_faint()),
                );
            }
            ui.label(
                egui::RichText::new("VyomKunji 0.3.0")
                    .font(Weight::Regular.font_id(10.0))
                    .color(text_faint()),
            );
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let col = self.status_level.color();
            ui.painter().circle_filled(
                ui.min_rect().left_center() + egui::vec2(5.0, 0.0),
                4.0,
                col,
            );
            ui.label(
                egui::RichText::new(&self.status)
                    .size(12.0)
                    .color(text_dim()),
            );
        });
    }

    fn view_header(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(self.nav.title())
                .font(Weight::Bold.font_id(23.0))
                .color(text()),
        );
        ui.label(
            egui::RichText::new(self.nav.hint())
                .size(12.0)
                .color(text_faint()),
        );
        ui.add_space(14.0);
    }

    fn view_requests(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        let info = self.pending.as_ref().map(|p| {
            (
                p.portal.clone(),
                p.origin.clone(),
                p.bytes,
                p.hash_hex.clone(),
                p.received.clone(),
                p.cert_cn.clone(),
                p.hash.name().to_string(),
                p.pkcs7,
            )
        });

        match info {
            Some((portal, origin, bytes, hash_hex, received, cert_cn, hash_name, pkcs7)) => {
                theme::card_frame(None).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Signing request")
                                .font(Weight::SemiBold.font_id(17.0))
                                .color(text()),
                        );
                        if let Some(p) = portals::find(&portal, &origin) {
                            chip(ui, p.name, accent());
                        } else {
                            chip(ui, &portal, accent());
                        }
                        if !self.queue.is_empty() {
                            chip(ui, &format!("+{} queued", self.queue.len()), warn());
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("received {}", received))
                                    .size(11.0)
                                    .color(text_faint()),
                            );
                        });
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    egui::Grid::new("reqmeta")
                        .num_columns(2)
                        .spacing([28.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("Payload")
                                    .color(text_faint())
                                    .size(12.0),
                            );
                            ui.label(
                                egui::RichText::new(format!("{} bytes", bytes))
                                    .size(12.5)
                                    .color(text()),
                            );
                            ui.end_row();
                            ui.label(egui::RichText::new("Origin").color(text_faint()).size(12.0));
                            if origin.is_empty() {
                                ui.label(
                                    egui::RichText::new("Local request")
                                        .size(12.5)
                                        .color(warn()),
                                );
                            } else {
                                let trusted = portals::origin_trusted(&origin);
                                ui.label(
                                    egui::RichText::new(if trusted {
                                        origin.clone()
                                    } else {
                                        format!("{} (unverified)", origin)
                                    })
                                    .family(egui::FontFamily::Monospace)
                                    .size(11.0)
                                    .color(if trusted {
                                        text_dim()
                                    } else {
                                        err()
                                    }),
                                );
                            }
                            ui.end_row();
                            ui.label(
                                egui::RichText::new(&hash_name)
                                    .color(text_faint())
                                    .size(12.0),
                            );
                            ui.label(
                                egui::RichText::new(&hash_hex)
                                    .family(egui::FontFamily::Monospace)
                                    .size(11.0)
                                    .color(text_dim()),
                            );
                            ui.end_row();
                            ui.label(egui::RichText::new("Output").color(text_faint()).size(12.0));
                            ui.label(
                                egui::RichText::new(if pkcs7 {
                                    "Detached PKCS#7 (CMS) with chain"
                                } else {
                                    "Raw token signature"
                                })
                                .size(12.5)
                                .color(text()),
                            );
                            ui.end_row();
                            ui.label(
                                egui::RichText::new("Certificate")
                                    .color(text_faint())
                                    .size(12.0),
                            );
                            ui.label(
                                egui::RichText::new(
                                    cert_cn.clone().unwrap_or_else(|| "auto-select".to_string()),
                                )
                                .size(12.5)
                                .color(text()),
                            );
                            ui.end_row();
                        });

                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("Token PIN")
                            .color(text_faint())
                            .size(12.0),
                    );
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut *self.pin)
                            .password(true)
                            .hint_text("Enter PIN to approve")
                            .desired_width(f32::INFINITY),
                    );
                    if r.changed() {
                        self.pin_set_at = if self.pin.is_empty() {
                            None
                        } else {
                            Some(Instant::now())
                        };
                    }

                    ui.add_space(14.0);
                    let signing = self.sign_rx.is_some();
                    ui.horizontal(|ui| {
                        let approve = ui.add_enabled(
                            !signing,
                            egui::Button::new(
                                egui::RichText::new(if signing {
                                    "Signing…"
                                } else {
                                    "Approve & Sign"
                                })
                                .font(Weight::SemiBold.font_id(13.5))
                                .color(egui::Color32::WHITE),
                            )
                            .fill(accent())
                            .min_size(egui::vec2(190.0, 40.0)),
                        );
                        if approve.clicked() {
                            self.approve_pending();
                        }
                        let cancel = ui.add_enabled(
                            !signing,
                            egui::Button::new("Cancel").min_size(egui::vec2(120.0, 40.0)),
                        );
                        if cancel.clicked() {
                            self.cancel();
                        }
                        if signing {
                            ui.add(egui::Spinner::new());
                        } else {
                            ui.label(
                                egui::RichText::new("Esc to cancel")
                                    .size(11.0)
                                    .color(text_faint()),
                            );
                        }
                    });
                });
            }
            None => {
                theme::card_frame(None).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(52.0, 52.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 26.0, theme::tint(ok(), 34));
                        ui.painter().circle_stroke(
                            rect.center(),
                            26.0,
                            egui::Stroke::new(1.0_f32, theme::tint(ok(), 110)),
                        );
                        let c = rect.center();
                        let check = vec![
                            c + egui::vec2(-7.0, 0.0),
                            c + egui::vec2(-2.5, 5.0),
                            c + egui::vec2(7.0, -6.0),
                        ];
                        ui.painter()
                            .add(egui::Shape::line(check, egui::Stroke::new(2.5_f32, ok())));
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("All clear")
                                .font(Weight::SemiBold.font_id(18.0))
                                .color(text()),
                        );
                        ui.label(
                            egui::RichText::new("No pending signing requests")
                                .size(12.5)
                                .color(text_dim()),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "When a portal asks for a DSC signature it will appear here for approval.",
                            )
                            .size(12.0)
                            .color(text_faint()),
                        );
                        ui.add_space(14.0);
                    });
                });

                if !self.history.is_empty() {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("Recent requests")
                            .font(Weight::SemiBold.font_id(13.0))
                            .color(text()),
                    );
                    ui.add_space(6.0);
                    theme::card_frame(None).show(ui, |ui| {
                        for h in self.history.iter().take(8) {
                            ui.label(
                                egui::RichText::new(h)
                                    .family(egui::FontFamily::Monospace)
                                    .size(11.5)
                                    .color(text_dim()),
                            );
                        }
                    });
                }
            }
        }
    }

    fn view_certificates(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        if self.certs.is_empty() {
            theme::card_frame(None).show(ui, |ui| {
                ui.label("No certificates found. Connect a DSC token and press Refresh.");
            });
            return;
        }

        let mut columns = 2;
        if ui.available_width() > 900.0 {
            columns = 3;
        }
        let card_w = (ui.available_width() - 16.0 * (columns as f32 - 1.0)) / columns as f32;

        egui::Grid::new("certgrid")
            .num_columns(columns)
            .spacing(egui::vec2(16.0, 16.0))
            .show(ui, |ui| {
                for (i, c) in self.certs.iter().enumerate() {
                    let selected = self.selected == i;
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(card_w, 128.0), egui::Sense::click());

                    let fill = if selected {
                        theme::tint(accent(), 30)
                    } else if resp.hovered() {
                        card_hi()
                    } else {
                        card()
                    };
                    ui.painter().rect_filled(rect, theme::radius(), fill);
                    ui.painter().rect_stroke(
                        rect,
                        theme::radius(),
                        egui::Stroke::new(1.0_f32, if selected { accent() } else { border() }),
                        egui::StrokeKind::Middle,
                    );

                    let pad = 14.0;
                    let top = rect.left_top() + egui::vec2(pad, pad);
                    let av = egui::Rect::from_min_size(top, egui::vec2(38.0, 38.0));
                    let name_col = if selected { accent() } else { accent2() };
                    ui.painter()
                        .circle_filled(av.center(), 19.0, theme::tint(name_col, 46));
                    ui.painter().text(
                        av.center(),
                        egui::Align2::CENTER_CENTER,
                        initials(&c.cn),
                        Weight::SemiBold.font_id(14.0),
                        name_col,
                    );

                    ui.painter().text(
                        av.right_top() + egui::vec2(12.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        if c.cn.is_empty() { "(unnamed)" } else { &c.cn },
                        Weight::SemiBold.font_id(13.5),
                        text(),
                    );
                    ui.painter().text(
                        av.right_top() + egui::vec2(12.0, 20.0),
                        egui::Align2::LEFT_TOP,
                        c.token_label.clone(),
                        Weight::Regular.font_id(10.5),
                        text_faint(),
                    );

                    let (exp_txt, exp_col) = match days_until(&c.not_after) {
                        Some(d) if d < 0 => (format!("expired {}d ago", -d), err()),
                        Some(d) if d < 30 => (format!("expires in {}d", d), warn()),
                        Some(d) => (format!("{}d left", d), ok()),
                        None => ("expiry unknown".to_string(), text_dim()),
                    };
                    let chip_font = Weight::Medium.font_id(10.5);
                    let galley =
                        ui.painter()
                            .layout_no_wrap(exp_txt.clone(), chip_font.clone(), exp_col);
                    let chip_size = galley.size() + egui::vec2(18.0, 8.0);
                    let chip_rect = egui::Rect::from_min_size(
                        rect.left_bottom() + egui::vec2(pad, -pad - chip_size.y),
                        chip_size,
                    );
                    ui.painter().rect_filled(
                        chip_rect,
                        theme::radius_sm(),
                        theme::tint(exp_col, 36),
                    );
                    ui.painter().text(
                        chip_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        exp_txt,
                        chip_font,
                        exp_col,
                    );

                    if selected {
                        ui.painter().circle_filled(
                            rect.right_top() + egui::vec2(-14.0, 14.0),
                            5.0,
                            accent(),
                        );
                    }

                    if resp.clicked() {
                        self.selected = i;
                    }
                    ui.end_row();
                }
            });

        ui.add_space(18.0);
        if let Some(c) = self.current_cert() {
            theme::card_frame(None).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let name = self
                        .current_cert()
                        .map(|c| c.cn.clone())
                        .unwrap_or_default();
                    avatar(ui, &initials(&name), 34.0, accent2());
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Certificate details")
                                .font(Weight::SemiBold.font_id(14.0))
                                .color(text()),
                        );
                        ui.label(
                            egui::RichText::new(if name.is_empty() {
                                "—".to_string()
                            } else {
                                name
                            })
                            .size(11.5)
                            .color(text_faint()),
                        );
                    });
                });
                ui.add_space(10.0);
                egui::Grid::new("certmeta")
                    .num_columns(2)
                    .spacing([28.0, 7.0])
                    .show(ui, |ui| {
                        kv(ui, "Subject", &c.cn, false);
                        ui.end_row();
                        kv(ui, "Issuer", &c.issuer, false);
                        ui.end_row();
                        kv(ui, "Serial", &c.serial, true);
                        ui.end_row();
                        kv(ui, "Valid until", &c.not_after, true);
                        ui.end_row();
                        kv(ui, "Token", &c.token_serial, true);
                        ui.end_row();
                        kv(ui, "CKA ID", &c.id, true);
                        ui.end_row();
                    });
            });
        }
    }

    fn view_sign(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        theme::card_frame(None).show(ui, |ui| {
            ui.label(
                egui::RichText::new("Manual signing")
                    .font(Weight::SemiBold.font_id(14.0))
                    .color(text()),
            );
            ui.label(
                egui::RichText::new("Sign a file or arbitrary text with your connected token.")
                    .size(12.0)
                    .color(text_faint()),
            );
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                ui.radio_value(&mut self.manual_source, ManualSource::File, "A file");
                ui.radio_value(&mut self.manual_source, ManualSource::Text, "Some text");
            });
            ui.add_space(8.0);

            match self.manual_source {
                ManualSource::File => {
                    ui.horizontal(|ui| {
                        if ui.button("Choose file…").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .set_title("Choose file to sign")
                                .pick_file()
                        {
                            self.manual_file_len =
                                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                            self.manual_file = Some(path);
                        }
                        match &self.manual_file {
                            Some(p) => {
                                let name = p
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                ui.label(egui::RichText::new(name).size(12.5).color(text()));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "({} bytes)",
                                        self.manual_file_len
                                    ))
                                    .size(11.5)
                                    .color(text_faint()),
                                );
                            }
                            None => {
                                ui.label(
                                    egui::RichText::new(
                                        "No file chosen (PDF, XML, JSON, anything)",
                                    )
                                    .size(12.0)
                                    .color(text_faint()),
                                );
                            }
                        }
                    });
                }
                ManualSource::Text => {
                    ui.label(
                        egui::RichText::new("Data to sign")
                            .color(text_faint())
                            .size(12.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.data_input)
                            .desired_width(f32::INFINITY)
                            .hint_text("text to sign"),
                    );
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Format").color(text_faint()).size(12.0));
                ui.radio_value(&mut self.manual_pkcs7, true, "Detached PKCS#7 (.p7s)");
                ui.radio_value(&mut self.manual_pkcs7, false, "Raw signature (.sig)");
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Hash").color(text_faint()).size(12.0));
                egui::ComboBox::from_id_salt("manual_hash")
                    .selected_text(self.manual_hash.name())
                    .show_ui(ui, |ui| {
                        for h in [
                            pkcs11::HashAlgorithm::Sha256,
                            pkcs11::HashAlgorithm::Sha384,
                            pkcs11::HashAlgorithm::Sha512,
                            pkcs11::HashAlgorithm::Sha1,
                        ] {
                            ui.selectable_value(&mut self.manual_hash, h, h.name());
                        }
                    });
            });

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Token PIN")
                    .color(text_faint())
                    .size(12.0),
            );
            let r = ui.add(
                egui::TextEdit::singleline(&mut *self.pin)
                    .password(true)
                    .hint_text("PIN")
                    .desired_width(f32::INFINITY),
            );
            if r.changed() {
                self.pin_set_at = if self.pin.is_empty() {
                    None
                } else {
                    Some(Instant::now())
                };
            }

            ui.add_space(14.0);
            let signing = self.sign_rx.is_some();
            ui.horizontal(|ui| {
                let b = ui.add_enabled(
                    !signing,
                    egui::Button::new(
                        egui::RichText::new(if signing {
                            "Signing…"
                        } else {
                            "Sign with selected certificate"
                        })
                        .font(Weight::SemiBold.font_id(13.0))
                        .color(egui::Color32::WHITE),
                    )
                    .fill(accent())
                    .min_size(egui::vec2(200.0, 38.0)),
                );
                if b.clicked() {
                    self.sign_manual();
                }
                if signing {
                    ui.add(egui::Spinner::new());
                } else if let Some(c) = self.current_cert() {
                    ui.label(
                        egui::RichText::new(format!("using {}", c.cn))
                            .size(11.5)
                            .color(text_dim()),
                    );
                }
            });
        });

        if !self.signature.is_empty() {
            ui.add_space(16.0);
            theme::card_frame(None).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Signature")
                            .font(Weight::SemiBold.font_id(14.0))
                            .color(text()),
                    );
                    chip(ui, &self.signature_cn, accent2());
                    if !self.signature_source.is_empty() {
                        ui.label(
                            egui::RichText::new(&self.signature_source)
                                .size(11.5)
                                .color(text_faint()),
                        );
                    }
                });
                if self.signature_demo {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Made with the demo key: this is not a valid signature.",
                        )
                        .size(12.0)
                        .color(warn()),
                    );
                }
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.signature)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let save_label = if self.signature_pkcs7 {
                        "Save .p7s…"
                    } else {
                        "Save .sig…"
                    };
                    if ui.button(save_label).clicked()
                        && let Ok(bytes) = B64.decode(&self.signature)
                    {
                        let ext = if self.signature_pkcs7 { "p7s" } else { "sig" };
                        let default_name = match &self.manual_file {
                            Some(p) => format!(
                                "{}.{ext}",
                                p.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "signature".to_string())
                            ),
                            None => format!("signature.{ext}"),
                        };
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Save signature")
                            .set_file_name(&default_name)
                            .save_file()
                        {
                            match std::fs::write(&path, bytes) {
                                Ok(()) => self.set_status(
                                    Level::Ok,
                                    format!("Saved signature to {}", path.display()),
                                ),
                                Err(e) => {
                                    self.set_status(Level::Err, format!("Could not save: {e}"))
                                }
                            }
                        }
                    }
                    if ui.button("Copy signature").clicked() {
                        ui.ctx().copy_text(self.signature.clone());
                    }
                });
            });
        }
    }

    fn view_portals(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        ui.label(
            egui::RichText::new(
                "The browser extension catches each portal's DSC sign button and localhost signer calls, and sends them here. No Java and no vendor utility are needed.",
            )
            .size(12.5)
            .color(text_dim()),
        );
        ui.add_space(12.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            let width = ui.available_width();
            let cols = if width > 760.0 {
                3
            } else if width > 480.0 {
                2
            } else {
                1
            };
            let card_w = ((width - 14.0 * (cols as f32 - 1.0)) / cols as f32).max(200.0);
            for row in portals::PORTALS.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    for portal in row {
                        let signed = self.portal_counts.get(portal.name).copied().unwrap_or(0);
                        theme::card_frame(None).show(ui, |ui| {
                            ui.set_width(card_w - 30.0);
                            ui.set_min_height(104.0);
                            ui.horizontal(|ui| {
                                avatar(
                                    ui,
                                    portal.short,
                                    34.0,
                                    if portal.filings { accent() } else { accent2() },
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(portal.name)
                                            .font(Weight::SemiBold.font_id(13.5))
                                            .color(text()),
                                    );
                                    ui.label(
                                        egui::RichText::new(portal.domains.join(", "))
                                            .family(egui::FontFamily::Monospace)
                                            .size(10.5)
                                            .color(text_faint()),
                                    );
                                });
                            });
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(portal.what)
                                    .size(12.0)
                                    .color(text_dim()),
                            );
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                chip(
                                    ui,
                                    if portal.filings {
                                        "Citizen filings"
                                    } else {
                                        "Procurement & trade"
                                    },
                                    if portal.filings { accent() } else { accent2() },
                                );
                                if signed > 0 {
                                    chip(ui, &format!("{signed} signed"), ok());
                                }
                            });
                        });
                    }
                });
                ui.add_space(10.0);
            }
        });
    }

    /// Build a privacy-conscious diagnostics blob for support requests. It
    /// deliberately excludes PINs, signatures, payload bytes, and certificate
    /// bodies; only versions, module paths, token metadata, and recent log text
    /// are included.
    fn diagnostics_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("VyomKunji {}\n", env!("CARGO_PKG_VERSION")));
        out.push_str(&format!(
            "OS: {} / {}\n",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        out.push_str(&format!(
            "Module: {}\n",
            self.manager
                .as_ref()
                .map(|(m, _)| m.as_str())
                .unwrap_or("(none)")
        ));
        out.push_str(&format!(
            "Browser bridge port: {}\n",
            self.bridge_port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "(not listening)".to_string())
        ));
        out.push_str(&format!("Certificates: {}\n", self.certs.len()));
        if let Some(cert) = self.certs.first() {
            out.push_str(&format!("Token: {}\n", cert.token_label));
        }
        out.push_str(&format!("Pending request: {}\n", self.pending.is_some()));
        out.push_str(&format!(
            "Status: {:?} — {}\n",
            self.status_level, self.status
        ));
        out.push_str("\nRecent log:\n");
        for entry in self.log.iter().rev().take(50) {
            out.push_str(&format!("{}  {}\n", entry.time, entry.text));
        }
        // Keep local usernames out of the copied text even when a custom PKCS#11
        // module path happens to live under the user profile.
        for (variable, placeholder) in [
            ("USERPROFILE", "%USERPROFILE%"),
            ("LOCALAPPDATA", "%LOCALAPPDATA%"),
            ("APPDATA", "%APPDATA%"),
        ] {
            if let Some(value) = std::env::var_os(variable) {
                let value = value.to_string_lossy();
                if !value.is_empty() {
                    out = out.replace(value.as_ref(), placeholder);
                }
            }
        }
        out
    }

    fn view_settings(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        theme::card_frame(None).show(ui, |ui| {
            ui.label(
                egui::RichText::new("PKCS#11 module")
                    .font(Weight::SemiBold.font_id(14.0))
                    .color(text()),
            );
            ui.label(
                egui::RichText::new(
                    "Override auto-detection with a specific module. Leave empty for auto-detect.",
                )
                .size(12.0)
                .color(text_faint()),
            );
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Module path")
                    .color(text_faint())
                    .size(12.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.module_input)
                    .desired_width(f32::INFINITY)
                    .hint_text("C:\\Windows\\System32\\...\\module.dll"),
            );

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    let p = self.module_input.trim().to_string();
                    let to_save = if p.is_empty() { None } else { Some(p.as_str()) };
                    let _ = pkcs11::save_configured_module(to_save);
                    self.selected_module = p.clone();
                    self.connect();
                }
                if ui.button("Browse for driver…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose PKCS#11 driver")
                        .add_filter("PKCS#11 library", &["dll", "so", "dylib"])
                        .pick_file()
                {
                    let p = path.to_string_lossy().to_string();
                    self.module_input = p.clone();
                    let _ = pkcs11::save_configured_module(Some(&p));
                    self.selected_module = p;
                    self.connect();
                }
                if ui.button("Auto-detect").clicked() {
                    self.module_input.clear();
                    self.selected_module.clear();
                    let _ = pkcs11::save_configured_module(None);
                    self.connect();
                }
                if ui.button("Refresh list").clicked() {
                    pkcs11::clear_unresponsive();
                    self.available_modules = pkcs11::rescan_modules();
                    self.connect();
                }
            });

            if !self.available_modules.is_empty() {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Discovered modules")
                        .color(text_faint())
                        .size(12.0),
                );
                egui::ComboBox::from_id_salt("modcombo")
                    .selected_text(if self.module_input.is_empty() {
                        "(auto-detect)".to_string()
                    } else {
                        self.module_input.clone()
                    })
                    .show_ui(ui, |ui| {
                        for m in self.available_modules.clone() {
                            if ui.selectable_label(self.module_input == m, &m).clicked() {
                                self.module_input = m;
                            }
                        }
                    });
            }

            ui.add_space(10.0);
            if let Some((m, _)) = &self.manager {
                kv(ui, "Active module", m, true);
            } else if self.demo_key {
                kv(ui, "Active module", "Demo key (no driver)", false);
            } else {
                kv(ui, "Active module", "(none)", false);
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("PIN & Window preferences")
                    .font(Weight::SemiBold.font_id(14.0))
                    .color(text()),
            );
            ui.add_space(6.0);
            if ui
                .checkbox(&mut self.remember_pin, "Remember PIN in memory between signatures")
                .changed()
            {
                let _ = pkcs11::save_config_key(
                    "rememberPin",
                    serde_json::json!(self.remember_pin),
                );
                if !self.remember_pin {
                    self.pin = Zeroizing::new(String::new());
                    self.pin_set_at = None;
                }
            }
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Forget PIN after")
                        .size(12.5)
                        .color(text_dim()),
                );
                let resp =
                    ui.add(egui::Slider::new(&mut self.pin_timeout_min, 1..=60).suffix(" min"));
                if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                    self.pin_timeout = Duration::from_secs(self.pin_timeout_min as u64 * 60);
                    let _ = pkcs11::save_config_key(
                        "pinTimeoutMin",
                        serde_json::json!(self.pin_timeout_min),
                    );
                }
            });
            if ui
                .checkbox(
                    &mut self.close_to_tray,
                    "Keep running in the tray when the window is closed",
                )
                .changed()
            {
                let _ = pkcs11::save_config_key(
                    "closeToTray",
                    serde_json::json!(self.close_to_tray),
                );
            }
            if ui
                .checkbox(
                    &mut self.demo_key,
                    format!("Use a demo key instead of a real one (PIN {DEMO_PIN}, signatures are not valid)"),
                )
                .changed()
            {
                let _ = pkcs11::save_config_key("demoKey", serde_json::json!(self.demo_key));
                self.connect();
            }

            ui.add_space(14.0);
            if ui.button("Copy diagnostics").clicked() {
                let text = self.diagnostics_text();
                ui.ctx().copy_text(text);
                self.set_status(
                    Level::Ok,
                    "Diagnostics copied to clipboard".to_string(),
                );
            }
            ui.label(
                egui::RichText::new(
                    "Includes version, token metadata, and the last 50 log lines. Never includes your PIN or signatures.",
                )
                .size(11.0)
                .color(text_faint()),
            );

            ui.add_space(14.0);
            if ui.button("Quit VyomKunji").clicked() {
                self.quit_requested = true;
                if let Some(ctx) = &self.ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        });
    }

    fn view_activity(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(self.nav.title())
                        .font(Weight::Bold.font_id(23.0))
                        .color(text()),
                );
                ui.label(
                    egui::RichText::new(self.nav.hint())
                        .size(12.0)
                        .color(text_faint()),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear").clicked() {
                    self.log.clear();
                }
                if ui.button("Copy").clicked() {
                    let text: String = self
                        .log
                        .iter()
                        .rev()
                        .map(|e| format!("{}  {}", e.time, e.text))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(text);
                }
            });
        });
        ui.add_space(14.0);

        theme::card_frame(None).show(ui, |ui| {
            if self.log.is_empty() {
                ui.label("Nothing logged yet.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for e in &self.log {
                    ui.horizontal(|ui| {
                        ui.painter().circle_filled(
                            ui.min_rect().left_center() + egui::vec2(4.0, 0.0),
                            3.5,
                            e.level.color(),
                        );
                        ui.label(
                            egui::RichText::new(&e.time)
                                .family(egui::FontFamily::Monospace)
                                .size(11.0)
                                .color(text_faint()),
                        );
                        ui.label(egui::RichText::new(&e.text).size(12.5).color(text_dim()));
                    });
                    ui.add_space(3.0);
                }
            });
        });
    }
}
