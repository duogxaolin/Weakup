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
  "nav.create": "Create",
  "nav.schedules": "Schedules",
  "nav.settings": "Settings",
  "nav.remote": "Devices",
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
  "create.description": "Set up a keep-awake or power-off schedule. Either one, or both together.",

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
  "powerOff.modeTime": "At a specific time",
  "powerOff.safetyNote": "A cancellable 60-second warning always appears before shutdown.",
  "powerOff.consentNote":
    "macOS will ask for permission to control System Events. That permission is what lets a scheduled shutdown run.",

  "field.duration": "Duration",
  "field.minutesSuffix": "min",
  "field.durationHint": "1 minute to 24 hours.",
  "field.localTime": "Local time",
  "field.date": "Date (optional)",
  "field.dateHint": "Leave empty for today, or tomorrow if the time has passed.",

  "jobs.eyebrow": "Activity",
  "jobs.heading": "Schedules",
  "jobs.description": "Every keep-awake and power-off job, with its live countdown and controls.",
  "jobs.loading": "Loading schedules…",
  "jobs.empty": "No schedules yet. Set one above and it will appear here.",

  "settings.eyebrow": "Preferences",
  "settings.heading": "Settings",
  "settings.description": "Appearance, language, and how Weakup starts and notifies you.",
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
  "settings.remoteControlTitle": "Allow remote control",
  // Says both halves out loud, because the pair of them is the decision the user is
  // making. "Only from this computer" is why an account alone is not enough; "does not
  // unpair" is why turning it off for an evening is safe.
  "settings.remoteControlDescription":
    "Let paired devices schedule a power-off here. Can only be changed on this computer, and turning it off does not unpair anything.",

  "remote.eyebrow": "Paired devices",
  "remote.heading": "Remote control",
  "remote.description": "Pair another device and let it schedule a power-off on this computer.",
  "remote.accountSignedOut": "Not signed in",
  "remote.accountSignedIn": "Signed in as {account}",
  "remote.thisDevice": "This computer",
  "remote.deviceId": "Device ID",
  "remote.signIn": "Sign in with Google",
  "remote.signOut": "Sign out",
  // Names the consequence, because a user who reads "sign out" may reasonably fear it
  // undoes their pairings.
  "remote.signOutNote": "Signing out leaves your paired devices in place.",
  "remote.showCode": "Show a pairing code",
  "remote.codeHeading": "Read this code out on the other device",
  "remote.codeExpiresIn": "Expires in {seconds} seconds",
  "remote.codeExpired": "This code has expired. Show a new one.",
  "remote.enterCodeLabel": "Code from the other device",
  "remote.enterCodePlaceholder": "6 characters",
  "remote.peerIdLabel": "That device's ID",
  "remote.peerKeyLabel": "That device's key",
  "remote.submitCode": "Pair this device",
  "remote.pairingsHeading": "Devices paired with this computer",
  "remote.pairingsEmpty": "No devices are paired yet. Show a code above to pair one.",
  "remote.devicesHeading": "Devices on this account",
  // The honest message for a machine with no relay configured: it cannot answer the
  // question, which is different from answering "none".
  "remote.devicesUnavailable":
    "This computer cannot reach the device list right now. Pairing by code still works.",
  "remote.revoke": "Unpair",
  "remote.revoked": "Unpaired",
  "remote.paired": "Paired",
  "remote.notPaired": "Not paired",
  // Three presence states, never two. A backgrounded phone is neither reachable nor gone,
  // and calling it either one misleads the user.
  "presence.online": "Online",
  "presence.stale": "May be asleep",
  "presence.offline": "Offline",
  "presence.neverSeen": "Never seen",
  "presence.silentFor": "Silent for {duration}",

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
  // Shown only if the OS refuses permission without saying why. The reason
  // normally comes from Rust, already written for a person.
  "permission.deniedFallback":
    "This computer will not let the app shut itself down, so a scheduled power-off cannot run.",  "error.bootFailed": "Weakup could not start up properly: {message}",

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

  "dashboard.description":
    "A quick look at what is keeping this computer awake and what runs next.",
  "dashboard.nextLabel": "Next scheduled",
  "dashboard.nothingScheduled": "Nothing is scheduled right now.",
  "dashboard.createLink": "Schedule something",
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
  "nav.create": "Tạo mới",
  "nav.schedules": "Lịch hẹn",
  "nav.settings": "Cài đặt",
  "nav.remote": "Thiết bị",
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
  "create.description": "Thiết lập lịch giữ máy thức hoặc tắt máy. Một trong hai, hoặc cả hai cùng lúc.",

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
  "powerOff.modeTime": "Vào một giờ cụ thể",
  "powerOff.safetyNote": "Luôn có cảnh báo 60 giây có thể hủy trước khi tắt máy.",
  "powerOff.consentNote":
    "macOS sẽ xin quyền điều khiển System Events. Quyền đó là thứ giúp lịch tắt máy chạy được.",

  "field.duration": "Thời lượng",
  "field.minutesSuffix": "phút",
  "field.durationHint": "Từ 1 phút đến 24 giờ.",
  "field.localTime": "Giờ địa phương",
  "field.date": "Ngày (không bắt buộc)",
  "field.dateHint": "Để trống nghĩa là hôm nay, hoặc mai nếu giờ đó đã qua.",

  "jobs.eyebrow": "Hoạt động",
  "jobs.heading": "Lịch hẹn",
  "jobs.description": "Mọi lịch giữ máy thức và tắt máy, kèm đồng hồ đếm ngược trực tiếp và các nút điều khiển.",
  "jobs.loading": "Đang tải lịch hẹn…",
  "jobs.empty": "Chưa có lịch hẹn nào. Hãy đặt một lịch ở trên và nó sẽ hiện ở đây.",

  "settings.eyebrow": "Tùy chọn",
  "settings.heading": "Cài đặt",
  "settings.description": "Giao diện, ngôn ngữ, và cách Weakup khởi động và thông báo cho bạn.",
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
  "settings.remoteControlTitle": "Cho phép điều khiển từ xa",
  "settings.remoteControlDescription":
    "Cho phép các thiết bị đã ghép đôi hẹn tắt máy này. Chỉ có thể thay đổi ngay trên máy này, và việc tắt tùy chọn sẽ không hủy ghép đôi thiết bị nào.",

  "remote.eyebrow": "Thiết bị đã ghép đôi",
  "remote.heading": "Điều khiển từ xa",
  "remote.description": "Ghép đôi một thiết bị khác và cho phép nó hẹn tắt máy tính này.",
  "remote.accountSignedOut": "Chưa đăng nhập",
  "remote.accountSignedIn": "Đã đăng nhập bằng {account}",
  "remote.thisDevice": "Máy này",
  "remote.deviceId": "Mã thiết bị",
  "remote.signIn": "Đăng nhập bằng Google",
  "remote.signOut": "Đăng xuất",
  "remote.signOutNote": "Đăng xuất vẫn giữ nguyên các thiết bị đã ghép đôi.",
  "remote.showCode": "Hiện mã ghép đôi",
  "remote.codeHeading": "Đọc mã này trên thiết bị kia",
  "remote.codeExpiresIn": "Hết hạn sau {seconds} giây",
  "remote.codeExpired": "Mã này đã hết hạn. Hãy tạo mã mới.",
  "remote.enterCodeLabel": "Mã từ thiết bị kia",
  "remote.enterCodePlaceholder": "6 ký tự",
  "remote.peerIdLabel": "Mã của thiết bị đó",
  "remote.peerKeyLabel": "Khóa của thiết bị đó",
  "remote.submitCode": "Ghép đôi thiết bị này",
  "remote.pairingsHeading": "Thiết bị đã ghép đôi với máy này",
  "remote.pairingsEmpty":
    "Chưa có thiết bị nào được ghép đôi. Hãy hiện mã ở trên để ghép đôi một thiết bị.",
  "remote.devicesHeading": "Thiết bị trong tài khoản này",
  "remote.devicesUnavailable":
    "Máy này hiện không lấy được danh sách thiết bị. Việc ghép đôi bằng mã vẫn hoạt động.",
  "remote.revoke": "Hủy ghép đôi",
  "remote.revoked": "Đã hủy ghép đôi",
  "remote.paired": "Đã ghép đôi",
  "remote.notPaired": "Chưa ghép đôi",
  "presence.online": "Đang trực tuyến",
  "presence.stale": "Có thể đang ngủ",
  "presence.offline": "Ngoại tuyến",
  "presence.neverSeen": "Chưa từng thấy",
  "presence.silentFor": "Đã im lặng {duration}",

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
  "permission.deniedFallback":
    "Máy này không cho phép ứng dụng tự tắt nguồn, nên lịch tắt máy sẽ không chạy được.",
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

  "dashboard.description":
    "Nhìn nhanh xem điều gì đang giữ máy tính này thức và việc gì sẽ chạy tiếp theo.",
  "dashboard.nextLabel": "Lịch kế tiếp",
  "dashboard.nothingScheduled": "Hiện chưa có lịch hẹn nào.",
  "dashboard.createLink": "Đặt một lịch hẹn",
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
