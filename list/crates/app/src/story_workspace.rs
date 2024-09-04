use gpui::*;
use prelude::FluentBuilder as _;
use private::serde::Deserialize;
use story::{ListStory, StoryContainer};
use workspace::TitleBar;

use std::sync::Arc;
use ui::{
    color_picker::{ColorPicker, ColorPickerEvent},
    dock::{DockArea, StackPanel, TabPanel},
    drawer::Drawer,
    modal::Modal,
    theme::{ActiveTheme, Colorize as _, Theme},
    Root, Sizable,
};

use crate::app_state::AppState;

#[derive(Clone, PartialEq, Eq, Deserialize)]
struct SelectLocale(SharedString);

actions!(workspace, [Open, CloseWindow]);

pub fn init(_app_state: Arc<AppState>, cx: &mut AppContext) {
    cx.on_action(|_action: &Open, _cx: &mut AppContext| {});

    Theme::init(cx);
    ui::init(cx);
    story::init(cx);
}

pub struct StoryWorkspace {
    dock_area: View<DockArea>,
}

impl StoryWorkspace {
    pub fn new(_app_state: Arc<AppState>, cx: &mut ViewContext<Self>) -> Self {
        cx.observe_window_appearance(|_workspace, cx| {
            Theme::sync_system_appearance(cx);
        })
        .detach();

        let stack_panel = cx.new_view(|cx| StackPanel::new(Axis::Horizontal, cx));
        let dock_area = cx.new_view(|cx| DockArea::new("main-dock", stack_panel.clone(), cx));
        let weak_dock_area = dock_area.downgrade();

        let center_tab_panel = cx.new_view(|cx| {
            let stack_panel = cx.new_view(|cx| StackPanel::new(Axis::Vertical, cx));
            TabPanel::new(Some(stack_panel), weak_dock_area.clone(), cx)
        });

        let left_tab_panel = cx.new_view(|cx| {
            let stack_panel = cx.new_view(|cx| StackPanel::new(Axis::Vertical, cx));
            TabPanel::new(Some(stack_panel), weak_dock_area.clone(), cx)
        });

        let right_tab_panel = cx.new_view(|cx| {
            let stack_panel = cx.new_view(|cx| StackPanel::new(Axis::Vertical, cx));
            TabPanel::new(Some(stack_panel), weak_dock_area.clone(), cx)
        });

        stack_panel.update(cx, |view, cx| {
            view.add_panel(
                left_tab_panel.clone(),
                Some(px(500.)),
                weak_dock_area.clone(),
                cx,
            );

            view.add_panel(center_tab_panel.clone(), None, weak_dock_area.clone(), cx);
            view.add_panel(
                right_tab_panel.clone(),
                Some(px(350.)),
                weak_dock_area.clone(),
                cx,
            );
        });

        StoryContainer::add_panel(
            "List",
            "A list displays a series of items.",
            ListStory::view(cx).into(),
            left_tab_panel.clone(),
            None,
            None,
            true,
            cx,
        );

        let theme_color_picker = cx.new_view(|cx| {
            let mut picker = ColorPicker::new("theme-color-picker", cx)
                .xsmall()
                .anchor(AnchorCorner::TopRight)
                .label("Primary Color");
            picker.set_value(cx.theme().primary, cx);
            picker
        });

        cx.subscribe(
            &theme_color_picker,
            |_, _, ev: &ColorPickerEvent, cx| match ev {
                ColorPickerEvent::Change(color) => {
                    if let Some(color) = color {
                        let theme = cx.global_mut::<Theme>();
                        theme.primary = *color;
                        theme.primary_hover = color.lighten(0.1);
                        theme.primary_active = color.darken(0.1);
                        cx.refresh();
                    }
                }
            },
        )
        .detach();

        Self { dock_area }
    }

    pub fn new_local(
        app_state: Arc<AppState>,
        cx: &mut AppContext,
    ) -> Task<anyhow::Result<WindowHandle<Root>>> {
        let window_bounds = Bounds::centered(None, size(px(1600.0), px(1200.0)), cx);

        cx.spawn(|mut cx| async move {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(9.0), px(9.0))),
                }),
                window_min_size: Some(gpui::Size {
                    width: px(640.),
                    height: px(480.),
                }),
                kind: WindowKind::Normal,
                ..Default::default()
            };

            let window = cx.open_window(options, |cx| {
                let story_view = cx.new_view(|cx| Self::new(app_state.clone(), cx));
                cx.new_view(|cx| Root::new(story_view.into(), cx))
            })?;

            window
                .update(&mut cx, |_, cx| {
                    cx.activate_window();
                    cx.set_window_title("GPUI App");
                    cx.on_release(|_, _, cx| {
                        // exit app
                        cx.quit();
                    })
                    .detach();
                })
                .expect("failed to update window");

            Ok(window)
        })
    }
}

pub fn open_new(
    app_state: Arc<AppState>,
    cx: &mut AppContext,
    init: impl FnOnce(&mut Root, &mut ViewContext<Root>) + 'static + Send,
) -> Task<()> {
    let task: Task<std::result::Result<WindowHandle<Root>, anyhow::Error>> =
        StoryWorkspace::new_local(app_state, cx);
    cx.spawn(|mut cx| async move {
        if let Some(root) = task.await.ok() {
            root.update(&mut cx, |workspace, cx| init(workspace, cx))
                .expect("failed to init workspace");
        }
    })
}

impl Render for StoryWorkspace {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let active_modal = Root::read(cx).active_modal.clone();
        let active_drawer = Root::read(cx).active_drawer.clone();
        let has_active_modal = active_modal.is_some();
        let notification_view = Root::read(cx).notification.clone();

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new("main-title", Box::new(CloseWindow))
                    .when(cfg!(not(windows)), |this| {
                        this.on_click(|event, cx| {
                            if event.up.click_count == 2 {
                                cx.zoom_window();
                            }
                        })
                    })
                    // left side
                    .child(div().flex().items_center().child("List Demo")),
            )
            .child(self.dock_area.clone())
            .when(!has_active_modal, |this| {
                this.when_some(active_drawer, |this, builder| {
                    let drawer = Drawer::new(cx);
                    this.child(builder(drawer, cx))
                })
            })
            .when_some(active_modal, |this, builder| {
                let modal = Modal::new(cx);
                this.child(builder(modal, cx))
            })
            .child(div().absolute().top_8().child(notification_view))
    }
}
