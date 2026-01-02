use eframe::egui;

use super::{state::{TwitchDeskApp, TemplatesEditorTab}, types::View};

impl TwitchDeskApp {
    pub(crate) fn ui_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar").show(ctx, |ui| {
            ui.heading("Menu");
            ui.separator();

            if ui.selectable_label(self.active_view == View::Home, "Home").clicked() {
                self.active_view = View::Home;
            }
            if ui
                .selectable_label(self.active_view == View::Settings, "Settings")
                .clicked()
            {
                self.active_view = View::Settings;
            }
            if ui
                .selectable_label(self.active_view == View::Channels, "Channels")
                .clicked()
            {
                self.active_view = View::Channels;
            }
            if ui
                .selectable_label(self.active_view == View::TwitchLookup, "Twitch lookup")
                .clicked()
            {
                self.active_view = View::TwitchLookup;
            }

            if ui
                .selectable_label(self.active_view == View::Templates, "Templates")
                .clicked()
            {
                self.active_view = View::Templates;
                // Auto-load templates when opening the view.
                self.templates_refresh_list();
            }
        });
    }

    pub(crate) fn ui_view(&mut self, ui: &mut egui::Ui) {
        match self.active_view {
            View::Home => {
                ui.heading("Dashboard");
                ui.label(&self.status);

                ui.add_space(8.0);
                ui.label("Connection");
                ui.horizontal(|ui| {
                    ui.label("API base URL");
                    ui.text_edit_singleline(&mut self.local.api_base_url);
                    if ui.button("Save").clicked() {
                        self.save_local();
                    }
                });
            }
            View::Settings => {
                ui.heading("Settings");
                ui.add_space(8.0);

                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label("Twitch bot credentials");
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label("Twitch Client ID");
                            ui.text_edit_singleline(&mut self.local.user_cfg.twitch_client_id);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Twitch Client Secret");
                            ui.add(
                                egui::TextEdit::singleline(
                                    &mut self.local.user_cfg.twitch_client_secret,
                                )
                                .password(true),
                            );
                        });

                        ui.add_space(8.0);
                        ui.checkbox(
                            &mut self.local.user_cfg.public_twitch_avatar_enabled,
                            "Enable public avatar endpoint",
                        );
                        if self.local.user_cfg.public_twitch_avatar_enabled {
                            let account = self
                                .local
                                .username
                                .as_deref()
                                .unwrap_or(self.username.as_str())
                                .trim();
                            ui.label(format!(
                                "URL: {}/{}/twitchavatar?username={{name}}",
                                self.local.api_base_url.trim().trim_end_matches('/'),
                                account
                            ));
                        }

                        ui.add_space(8.0);
                        if ui.button("Save").clicked() {
                            self.save_local();
                            self.save_user_config_to_api();
                        }
                    });
            }
            View::Channels => {
                ui.heading("Channels");
                ui.label("Add Twitch channels and see live status");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.channel_to_add)
                            .hint_text("e.g. shroud")
                            .desired_width(240.0),
                    );
                    if ui.button("Add").clicked() {
                        self.add_channel();
                    }
                    if ui.button("Refresh").clicked() {
                        self.refresh_channel_statuses();
                    }
                });

                ui.add_space(8.0);
                if self.channel_statuses.is_empty() {
                    ui.label("No channels yet");
                } else {
                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::same(12.0))
                        .show(ui, |ui| {
                            for ch in self.channel_statuses.clone() {
                                ui.horizontal(|ui| {
                                    let dot = if ch.is_live {
                                        egui::RichText::new("●").color(egui::Color32::GREEN)
                                    } else {
                                        egui::RichText::new("●").color(egui::Color32::RED)
                                    };
                                    ui.label(dot);
                                    ui.label(ch.login.clone());

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Remove").clicked() {
                                                self.remove_channel(&ch.login);
                                            }
                                        },
                                    );
                                });
                            }
                        });
                }
            }
            View::TwitchLookup => {
                ui.heading("Twitch lookup");
                ui.label("Test: call cloud API Twitch proxy");
                ui.horizontal(|ui| {
                    ui.label("login");
                    ui.text_edit_singleline(&mut self.test_login);
                    if ui.button("Fetch").clicked() {
                        self.test_twitch_lookup();
                    }
                });
                ui.text_edit_multiline(&mut self.test_result);
            }

            View::Templates => {
                self.ui_templates(ui);
            }
        }
    }

    fn ui_templates(&mut self, ui: &mut egui::Ui) {
        ui.heading("Templates");
        if !self.templates_status.trim().is_empty() {
            ui.label(self.templates_status.clone());
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.templates_refresh_list();
            }

            ui.separator();

            ui.label("New template");
            ui.add(
                egui::TextEdit::singleline(&mut self.templates_new_name)
                    .hint_text("e.g. Alerts")
                    .desired_width(180.0),
            );
            if ui.button("Create").clicked() {
                self.templates_create();
            }
        });

        ui.add_space(10.0);

        // Split layout: fixed list on the left, editor on the right.
        ui.horizontal(|ui| {
            let list_width = 280.0;
            let height = ui.available_height();

            ui.allocate_ui_with_layout(
                egui::vec2(list_width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.heading("Your templates");
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.templates_list.is_empty() {
                            ui.label("No templates yet");
                            return;
                        }
                        for t in self.templates_list.clone() {
                            let selected = self
                                .templates_selected_template_id
                                .as_deref()
                                .map(|id| id == t.id)
                                .unwrap_or(false);
                            if ui.selectable_label(selected, t.name.clone()).clicked() {
                                self.templates_select_template(&t.id);
                            }
                        }
                    });
                },
            );

            ui.separator();

            ui.vertical(|ui| {
                ui.heading("Editor");
                ui.add_space(6.0);

                let Some(template_id) = self.templates_selected_template_id.clone() else {
                    ui.label("Select a template to edit.");
                    return;
                };

                let template_name = self
                    .templates_selected_template_name
                    .clone()
                    .unwrap_or_else(|| "<unknown>".to_string());
                ui.label(format!("Template: {}", template_name));

                ui.horizontal(|ui| {
                ui.label("Version");
                let mut selected = self.templates_selected_version.clone().unwrap_or_default();
                egui::ComboBox::from_id_salt("templates_version_combo")
                    .selected_text(if selected.is_empty() {
                        "<none>".to_string()
                    } else {
                        selected.clone()
                    })
                    .show_ui(ui, |ui| {
                        for v in self.templates_versions.clone() {
                            let tag = if v.is_published { " (published)" } else { "" };
                            let text = format!("{}{}", v.version, tag);
                            ui.selectable_value(&mut selected, v.version.clone(), text);
                        }
                    });

                if selected != self.templates_selected_version.clone().unwrap_or_default() {
                    if !selected.trim().is_empty() {
                        self.templates_load_version(&template_id, &selected);
                    }
                }
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.templates_save_current_version();
                }
                if ui.button("Publish").clicked() {
                    self.templates_publish_current_version();
                }
                });

                ui.add_space(6.0);
                if let (Some(ver), Some(username)) = (
                self.templates_selected_version.clone(),
                self.local
                    .username
                    .clone()
                    .or_else(|| Some(self.username.clone()))
                    .filter(|s| !s.trim().is_empty()),
                ) {
                    let base = self.local.api_base_url.trim().trim_end_matches('/');
                    let url = format!(
                        "{}/{}/template/{}/{}",
                        base,
                        urlencoding::encode(username.trim()),
                        urlencoding::encode(template_name.trim()),
                        urlencoding::encode(ver.trim())
                    );
                    ui.horizontal(|ui| {
                        ui.label("Preview URL");
                        ui.label(url.clone());
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Copy (mock)").clicked() {
                            ui.output_mut(|o| o.copied_text = format!("{}?mock=true", url));
                            self.templates_status = "Copied preview URL.".to_string();
                        }

                        if ui.button("Preview").clicked() {
                            match crate::preview::open_preview(&url) {
                                Ok(()) => self.templates_status = "Opened in-app preview.".to_string(),
                                Err(e) => self.templates_status = format!("Preview failed: {e:#}"),
                            }
                        }

                        if ui.button("Preview (mock)").clicked() {
                            let u = format!("{}?mock=true", url);
                            match crate::preview::open_preview(&u) {
                                Ok(()) => {
                                    self.templates_status = "Opened in-app preview (mock).".to_string()
                                }
                                Err(e) => self.templates_status = format!("Preview failed: {e:#}"),
                            }
                        }

                        if ui.button("Open").clicked() {
                            match webbrowser::open(&url) {
                                Ok(_) => self.templates_status = "Opened preview URL.".to_string(),
                                Err(e) => self.templates_status = format!("Open failed: {e}"),
                            }
                        }

                        if ui.button("Open (mock)").clicked() {
                            let u = format!("{}?mock=true", url);
                            match webbrowser::open(&u) {
                                Ok(_) => self.templates_status = "Opened preview URL (mock).".to_string(),
                                Err(e) => self.templates_status = format!("Open failed: {e}"),
                            }
                        }
                    });
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                ui.label("New version");
                ui.add(
                    egui::TextEdit::singleline(&mut self.templates_new_version)
                        .hint_text("e.g. 2")
                        .desired_width(120.0),
                );
                if ui.button("Create from current").clicked() {
                    self.templates_create_version_from_current();
                }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                ui.label("Duplicate template");
                ui.add(
                    egui::TextEdit::singleline(&mut self.templates_duplicate_template_name)
                        .hint_text("e.g. Alerts Copy")
                        .desired_width(160.0),
                );
                if ui.button("Duplicate").clicked() {
                    self.templates_duplicate_template();
                }
                });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                let html_sel = self.templates_editor_tab == TemplatesEditorTab::Html;
                let css_sel = self.templates_editor_tab == TemplatesEditorTab::Css;
                let js_sel = self.templates_editor_tab == TemplatesEditorTab::Js;

                if ui.selectable_label(html_sel, "HTML").clicked() {
                    self.templates_editor_tab = TemplatesEditorTab::Html;
                }
                if ui.selectable_label(css_sel, "CSS").clicked() {
                    self.templates_editor_tab = TemplatesEditorTab::Css;
                }
                if ui.selectable_label(js_sel, "JS").clicked() {
                    self.templates_editor_tab = TemplatesEditorTab::Js;
                }
                });

                ui.add_space(6.0);
                match self.templates_editor_tab {
                    TemplatesEditorTab::Html => {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.templates_index_html)
                                .desired_rows(18)
                                .code_editor(),
                        );
                    }
                    TemplatesEditorTab::Css => {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.templates_style_css)
                                .desired_rows(18)
                                .code_editor(),
                        );
                    }
                    TemplatesEditorTab::Js => {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.templates_overlay_js)
                                .desired_rows(18)
                                .code_editor(),
                        );
                    }
                }
            });
        });
    }
}
