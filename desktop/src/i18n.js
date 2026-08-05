/*
 * The interface's string tables.
 *
 * Pure data and two lookups, so `i18n.test.js` can assert the tables agree without a
 * window. Nothing here touches `document` or `invoke`.
 *
 * Two rules hold the file together.
 *
 * English is the reference. Every other language must define exactly the same keys, and a
 * test enforces it — a missing key would otherwise render as the raw key ("job.pause") in
 * the middle of an otherwise translated window, and a stray extra key is a rename that only
 * landed in one table.
 *
 * Placeholders are named, not positional. `{count}` survives a translator reordering a
 * sentence; `%s` does not, and the reordering is exactly what translation does.
 */

/**
 * Substitutes `{name}` placeholders.
 *
 * An unknown placeholder is left as written rather than replaced with "undefined": the
 * literal `{count}` in the window is visibly a bug, whereas "undefined seconds left" reads
 * like a real, if broken, message.
 */
function fill(template, values) {
  if (!values) return template;
  return template.replace(/\{(\w+)\}/g, (whole, key) =>
    Object.prototype.hasOwnProperty.call(values, key) ? String(values[key]) : whole,
  );
}

const en = {
  "app.tagline": "Keep control of sleep and shutdown.",
  "app.skipToScheduling": "Skip to scheduling",

  "nav.section": "Sections",
  "nav.dashboard": "Dashboard",
  "nav.schedules": "Schedules",
  "nav.settings": "Settings",
  "nav.trayActive": "Tray active",
  "nav.runsInBackground": "Runs in the background",

  "grace.eyebrow": "Power-off warning",
  "grace.heading": "This computer is shutting down",
  "grace.secondsLeft": "seconds left",
  "grace.secondLeft": "second left",
  "grace.cancel": "Cancel shutdown and keep this computer on",
  "grace.note": "Cancelling is safe. The scheduled power-off job will stop.",
  "grace.valueText": "{remaining} until this computer shuts down",

  "status.eyebrow": "System status",
  "status.checking": "Checking your schedules…",
  "status.backgroundNote": "Weakup keeps working when this window is closed.",

  "degraded.eyebrow": "Needs attention",
  "degraded.heading": "Some features are unavailable",

  "create.eyebrow": "New schedule",
  "create.heading": "Choose what Weakup should do",

  "keepAwake.title": "Keep awake",
  "keepAwake.description": "Prevent the display from going to sleep",
  "keepAwake.modeLabel": "Keep awake until",
  "keepAwake.modeIndefinite": "I stop it",
  "keepAwake.modeDuration": "A duration",
  "keepAwake.modeTime": "A time today",

  "powerOff.title": "Power off",
  "powerOff.description": "Shut down this computer at a safe time",
  "powerOff.modeLabel": "Power off",
  "powerOff.modeDuration": "After a duration",
  "powerOff.modeTime": "At a time today",
  "powerOff.safetyNote": "A cancellable 60-second warning always appears before shutdown.",

  "field.duration": "Duration",
  "field.minutesSuffix": "min",
  "field.durationHint": "1 minute to 24 hours.",
  "field.localTime": "Local time",

  "jobs.eyebrow": "Activity",
  "jobs.heading": "Schedules",
  "jobs.loading": "Loading schedules…",
  "jobs.empty": "No schedules yet. Set one above and it will appear here.",

  "settings.eyebrow": "Preferences",
  "settings.heading": "Settings",
  "settings.timezoneLabel": "Timezone for scheduled times",
  "settings.timezoneHint": "Changing this moves existing jobs scheduled for a time of day.",
  "settings.appearanceLabel": "Appearance",
  "settings.appearanceHint": "Choose your preferred colour theme.",
  "settings.themeAuto": "Auto",
  "settings.themeLight": "Light",
  "settings.themeDark": "Dark",
  "settings.languageLabel": "Language",
  "settings.languageHint": "The language this window is written in.",
  "settings.notificationsTitle": "Notifications",
  "settings.notificationsDescription": "Show job and shutdown warnings",
  "settings.autostartTitle": "Launch at login",
  "settings.autostartDescription": "Start hidden in the menu bar or tray",

  "footer.title": "Weakup runs in the background",
  "footer.description": "Schedules continue after this window closes.",
  "footer.hide": "Close to tray",
  "footer.hideUnavailable": "Close to the tray (no tray on this computer)",

  "quit.heading": "Quit Weakup?",
  "quit.keepRunning": "Keep running",
  "quit.confirm": "Quit anyway",
  "replace.heading": "Replace the active schedule?",
  "replace.keepCurrent": "Keep current",
  "replace.confirm": "Replace schedule",

  "job.pause": "Pause",
  "job.resume": "Resume",
  "job.cancel": "Cancel",

  "error.chooseOne": "Choose at least one of the two.",
  "error.checkTime": "Check the time or the number of minutes.",
  "error.bootFailed": "Weakup could not start up properly: {message}",

  "selection.both": "Keep awake + power off",
  "selection.bothSubmit": "Start both schedules",
  "selection.keepAwake": "Keep the display awake",
  "selection.keepAwakeSubmit": "Start keeping awake",
  "selection.powerOff": "Schedule a safe power-off",
  "selection.powerOffSubmit": "Schedule power-off",
  "selection.none": "Choose one or both",
  "selection.noneSubmit": "Choose an action to start",

  "preview.indefinite": "Runs until you turn it off.",
  "preview.powerOff": "Shuts down at {at} ({remaining} from now).",
  "preview.keepAwake": "Stays awake until {at} ({remaining} from now).",

  "time.hour": "hour",
  "time.hours": "hours",
  "time.minute": "minute",
  "time.minutes": "minutes",
  "time.second": "second",
  "time.seconds": "seconds",
  "time.at": "{time} on {date}",

  "countdown.overdue": "Was due at {at}. It did not run.",
  "countdown.past": "Was set for {at}.",
  "countdown.paused": "Paused with {remaining} left.",
  "countdown.due": "Due now.",
  "countdown.left": "{remaining} left — {at}.",

  "dashboard.attentionOne": "1 schedule needs attention",
  "dashboard.attentionMany": "{count} schedules need attention",
  "dashboard.attentionDetail":
    "Review the schedule details below. No missed power-off runs without warning.",
  "dashboard.activeOne": "{label} is active",
  "dashboard.activeMany": "{count} schedules are active",
  "dashboard.activeDetail": "Weakup is monitoring the schedule in the background.",
  "dashboard.activePausedNote": "{count} more paused.",
  "dashboard.pausedOne": "1 schedule is paused",
  "dashboard.pausedMany": "{count} schedules are paused",
  "dashboard.pausedDetail": "Resume a schedule below when you are ready.",
  "dashboard.idle": "No active schedules",
  "dashboard.idleDetail":
    "Choose an action below. Weakup keeps working when this window is closed.",

  "settings.saved": "Saved.",
  "settings.savedMoved":
    "Saved. {count} job{plural} set for a time of day moved to the new timezone.",
  "settings.autostartOn": "Weakup will start when you log in.",
  "settings.autostartOff": "Weakup will not start when you log in.",

  "quit.activeJob":
    "A job is still running. Quitting cancels it, and this computer will not shut down on its own.",
  "quit.nothingScheduled": "Nothing is scheduled right now.",

  "degraded.reported": "Reported: {reason}",
  "degraded.essential":
    "{label} is not working on this computer — this is one of the two things Weakup does.",
  "degraded.nonEssential": "{label} is not working on this computer.",
  "consequence.keep_awake":
    "Screen-awake jobs will not hold the screen on. Nothing in Weakup can change that on this computer.",
  "consequence.power_off":
    "A scheduled shutdown will not run. The job will be recorded as failed instead, and this computer will stay on.",
  "consequence.tray":
    "There is no icon to reach Weakup with, so closing this window will ask before quitting rather than hiding.",
  "consequence.autostart":
    "Weakup will not start when you log in, so a job set for tomorrow will not run unless Weakup is already open.",
  "consequence.notifications":
    "No notifications, including the warning before a shutdown. The countdown will appear in this window only.",
};

/*
 * Vietnamese.
 *
 * Two things are deliberately not translated word-for-word. The plural helpers are single
 * words because Vietnamese does not inflect for number, so `formatRemaining` composing
 * "2 giờ" needs no separate plural form — the same string serves both, and the test that
 * every key exists is what keeps them present rather than quietly dropped.
 *
 * The shutdown copy is also blunter than the English. "Máy tính sẽ tắt" states the fact
 * before the countdown does; softening an irreversible action into a suggestion is the one
 * place a translation choice could cost the user their work.
 */
const vi = {
  "app.tagline": "Kiểm soát chế độ ngủ và tắt máy.",
  "app.skipToScheduling": "Chuyển đến phần đặt lịch",

  "nav.section": "Mục",
  "nav.dashboard": "Tổng quan",
  "nav.schedules": "Lịch hẹn",
  "nav.settings": "Cài đặt",
  "nav.trayActive": "Đang hoạt động trong khay hệ thống",
  "nav.runsInBackground": "Ứng dụng vẫn chạy ngầm khi đóng cửa sổ",

  "grace.eyebrow": "Cảnh báo tắt máy",
  "grace.heading": "Máy tính này sẽ tắt",
  "grace.secondsLeft": "giây còn lại",
  "grace.secondLeft": "giây còn lại",
  "grace.cancel": "Hủy tắt máy và giữ máy tính hoạt động",
  "grace.note": "Hủy là an toàn. Lịch tắt máy đã hẹn sẽ dừng lại.",
  "grace.valueText": "còn {remaining} trước khi máy tính tắt",

  "status.eyebrow": "Trạng thái hệ thống",
  "status.checking": "Đang kiểm tra lịch hẹn…",
  "status.backgroundNote": "Weakup vẫn hoạt động khi cửa sổ này đóng.",

  "degraded.eyebrow": "Cần chú ý",
  "degraded.heading": "Một số tính năng không khả dụng",

  "create.eyebrow": "Lịch hẹn mới",
  "create.heading": "Chọn việc Weakup sẽ làm",

  "keepAwake.title": "Giữ máy thức",
  "keepAwake.description": "Ngăn màn hình chuyển sang chế độ ngủ",
  "keepAwake.modeLabel": "Giữ thức đến khi",
  "keepAwake.modeIndefinite": "Tôi tự dừng",
  "keepAwake.modeDuration": "Hết một khoảng thời gian",
  "keepAwake.modeTime": "Đến một giờ hôm nay",

  "powerOff.title": "Tắt máy",
  "powerOff.description": "Tắt máy tính này vào thời điểm an toàn",
  "powerOff.modeLabel": "Tắt máy",
  "powerOff.modeDuration": "Sau một khoảng thời gian",
  "powerOff.modeTime": "Vào một giờ hôm nay",
  "powerOff.safetyNote": "Luôn có cảnh báo 60 giây có thể hủy trước khi tắt máy.",

  "field.duration": "Thời lượng",
  "field.minutesSuffix": "phút",
  "field.durationHint": "Từ 1 phút đến 24 giờ.",
  "field.localTime": "Giờ địa phương",

  "jobs.eyebrow": "Hoạt động",
  "jobs.heading": "Lịch hẹn",
  "jobs.loading": "Đang tải lịch hẹn…",
  "jobs.empty": "Chưa có lịch hẹn nào. Hãy đặt một lịch ở trên và nó sẽ hiện ở đây.",

  "settings.eyebrow": "Tùy chọn",
  "settings.heading": "Cài đặt",
  "settings.timezoneLabel": "Múi giờ cho các lịch hẹn theo giờ",
  "settings.timezoneHint": "Thay đổi mục này sẽ dịch chuyển các lịch hẹn theo giờ trong ngày.",
  "settings.appearanceLabel": "Giao diện",
  "settings.appearanceHint": "Chọn bảng màu bạn thích.",
  "settings.themeAuto": "Tự động",
  "settings.themeLight": "Sáng",
  "settings.themeDark": "Tối",
  "settings.languageLabel": "Ngôn ngữ",
  "settings.languageHint": "Ngôn ngữ hiển thị của cửa sổ này.",
  "settings.notificationsTitle": "Thông báo",
  "settings.notificationsDescription": "Hiện thông báo lịch hẹn và cảnh báo tắt máy",
  "settings.autostartTitle": "Mở khi đăng nhập",
  "settings.autostartDescription": "Khởi động ẩn ở thanh menu hoặc khay hệ thống",

  "footer.title": "Weakup chạy ở chế độ nền",
  "footer.description": "Các lịch hẹn vẫn tiếp tục sau khi đóng cửa sổ này.",
  "footer.hide": "Đóng vào khay",
  "footer.hideUnavailable": "Đóng vào khay (máy này không có khay hệ thống)",

  "quit.heading": "Thoát Weakup?",
  "quit.keepRunning": "Tiếp tục chạy",
  "quit.confirm": "Vẫn thoát",
  "replace.heading": "Thay thế lịch hẹn đang chạy?",
  "replace.keepCurrent": "Giữ lịch hiện tại",
  "replace.confirm": "Thay thế lịch hẹn",

  "job.pause": "Tạm dừng",
  "job.resume": "Tiếp tục",
  "job.cancel": "Hủy",

  "error.chooseOne": "Hãy chọn ít nhất một trong hai.",
  "error.checkTime": "Hãy kiểm tra lại giờ hoặc số phút.",
  "error.bootFailed": "Weakup không thể khởi động đúng cách: {message}",

  "selection.both": "Giữ máy thức + tắt máy",
  "selection.bothSubmit": "Bắt đầu cả hai lịch hẹn",
  "selection.keepAwake": "Giữ màn hình luôn sáng",
  "selection.keepAwakeSubmit": "Bắt đầu giữ máy thức",
  "selection.powerOff": "Hẹn tắt máy an toàn",
  "selection.powerOffSubmit": "Hẹn tắt máy",
  "selection.none": "Chọn một hoặc cả hai",
  "selection.noneSubmit": "Chọn một hành động để bắt đầu",

  "preview.indefinite": "Chạy đến khi bạn tự tắt.",
  "preview.powerOff": "Tắt máy vào {at} (còn {remaining} nữa).",
  "preview.keepAwake": "Giữ thức đến {at} (còn {remaining} nữa).",

  "time.hour": "giờ",
  "time.hours": "giờ",
  "time.minute": "phút",
  "time.minutes": "phút",
  "time.second": "giây",
  "time.seconds": "giây",
  "time.at": "{time} ngày {date}",

  "countdown.overdue": "Đã đến hạn lúc {at}. Lịch này đã không chạy.",
  "countdown.past": "Đã được hẹn cho {at}.",
  "countdown.paused": "Đang tạm dừng, còn {remaining}.",
  "countdown.due": "Đến hạn ngay bây giờ.",
  "countdown.left": "còn {remaining} — {at}.",

  "dashboard.attentionOne": "1 lịch hẹn cần chú ý",
  "dashboard.attentionMany": "{count} lịch hẹn cần chú ý",
  "dashboard.attentionDetail":
    "Hãy xem chi tiết lịch hẹn bên dưới. Không có lần tắt máy nào bị bỏ qua mà không cảnh báo.",
  "dashboard.activeOne": "{label} đang hoạt động",
  "dashboard.activeMany": "{count} lịch hẹn đang hoạt động",
  "dashboard.activeDetail": "Weakup đang theo dõi lịch hẹn ở chế độ nền.",
  "dashboard.activePausedNote": "{count} lịch khác đang tạm dừng.",
  "dashboard.pausedOne": "1 lịch hẹn đang tạm dừng",
  "dashboard.pausedMany": "{count} lịch hẹn đang tạm dừng",
  "dashboard.pausedDetail": "Tiếp tục một lịch hẹn bên dưới khi bạn đã sẵn sàng.",
  "dashboard.idle": "Không có lịch hẹn nào đang chạy",
  "dashboard.idleDetail":
    "Hãy chọn một hành động bên dưới. Weakup vẫn hoạt động khi cửa sổ này đóng.",

  "settings.saved": "Đã lưu.",
  "settings.savedMoved":
    "Đã lưu. {count} lịch hẹn{plural} theo giờ trong ngày đã chuyển sang múi giờ mới.",
  "settings.autostartOn": "Weakup sẽ mở khi bạn đăng nhập.",
  "settings.autostartOff": "Weakup sẽ không mở khi bạn đăng nhập.",

  "quit.activeJob":
    "Một lịch hẹn vẫn đang chạy. Thoát sẽ hủy lịch đó, và máy tính này sẽ không tự tắt.",
  "quit.nothingScheduled": "Hiện không có lịch hẹn nào.",

  "degraded.reported": "Hệ thống báo: {reason}",
  "degraded.essential":
    "{label} không hoạt động trên máy này — đây là một trong hai việc chính của Weakup.",
  "degraded.nonEssential": "{label} không hoạt động trên máy này.",
  "consequence.keep_awake":
    "Các lịch giữ màn hình sáng sẽ không giữ được màn hình. Weakup không thể thay đổi điều đó trên máy này.",
  "consequence.power_off":
    "Lịch tắt máy đã hẹn sẽ không chạy. Lịch đó sẽ được ghi là thất bại, và máy tính này vẫn bật.",
  "consequence.tray":
    "Không có biểu tượng nào để mở lại Weakup, nên đóng cửa sổ này sẽ hỏi trước khi thoát thay vì ẩn đi.",
  "consequence.autostart":
    "Weakup sẽ không mở khi bạn đăng nhập, nên lịch hẹn cho ngày mai sẽ không chạy trừ khi Weakup đã mở.",
  "consequence.notifications":
    "Không có thông báo nào, kể cả cảnh báo trước khi tắt máy. Đồng hồ đếm ngược chỉ hiện trong cửa sổ này.",
};

export const TABLES = { en, vi };

/** The languages the settings picker offers, with names written in the language itself. */
export const LANGUAGES = [
  { code: "en", label: "English (English)" },
  { code: "vi", label: "Vietnamese (Tiếng Việt)" },
];

export const DEFAULT_LANGUAGE = "en";

/**
 * Builds a `t(key, values)` for one language.
 *
 * Falls through to English for a key the language lacks, rather than showing the key. The
 * key-parity test means that should never happen in a shipped build, but a fallback that
 * degrades to a readable English sentence is better than one that degrades to `job.pause`.
 */
export function translator(language) {
  const table = TABLES[language] ?? TABLES[DEFAULT_LANGUAGE];

  return function t(key, values) {
    const template = table[key] ?? TABLES[DEFAULT_LANGUAGE][key];
    if (template === undefined) return key;
    return fill(template, values);
  };
}

/** Whether a language code has a table, used to sanitise a stored or IPC value. */
export function isSupportedLanguage(code) {
  return Object.prototype.hasOwnProperty.call(TABLES, code);
}
