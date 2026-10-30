# 实现结构

## 技术栈

| 部分 | 实现与作用 |
|---|---|
| 主程序 | Rust 2024；错误类型、身份、缓存、提醒周期与串行工作线程 |
| 窗口与托盘 | `windows-sys` 调Win32/GDI/Shell；原生窗口和控件语义，自绘像素；不加载网页运行时 |
| 2.4G查询 | HID++ 使用 `hidapi` Windows native 后端；N3、Compx、Razer、PELTA、AULA、AJAZZ、小米及DELUX状态通道使用有界原生重叠I/O；报文只在适配器，界面不接触协议 |
| 蓝牙 | `windows` / WinRT访问标准GATT Battery Service |
| 自启 | 用户Startup的`.lnk`，Shell Link COM；用户禁用后不自行恢复 |
| 图标 | 透明掩码和ICO编入EXE；Python/Pillow仅用于开发期再生成 |
| 构建 | Cargo+MSVC+Windows SDK，静态CRT，release单EXE；许可声明也编入二进制 |

## 数据路径

设备发现 → `ProtocolAdapter` / `Registry` → 通用 `Reading` → `Manager` → 主窗口、托盘、提醒。

- `src/device/`：发现、协议、查询、取消、设备类型与读数。`ADAPTERS`静态登记协议家族，不是设备数据库或运行时插件。
- `src/device/discovery.rs`：先收集 hidapi 可见集合、按物理身份分组并缓存兄弟集合，再确定输入候选；按稳定顺序选择名字，混合鼠标/键盘归 Generic，vendor-only 不作为鼠标。元数据不等于品牌、无线制式或电量能力。
- `src/device/support.rs`：按需适配报告，软件/系统信息、监控快照、静态匹配原因和所选HID组/BAS证据分层；2MiB上限，最多32个相关HID集合。默认去掉路径、容器和地址，错误文本去路径；只在用户选择位置后原子写UTF-8文件。
- `src/device/diagnostics.rs`：独立 SetupDi HID 接口清单，访问失败仍保留端点；按需零访问权限查询原生 caps，无输入流、配置或协议流量。默认报告使用局部分组标签，显式标识符导出才附路径/容器；有界文本、数量、时间和取消，失败与截断显式报告。
- `src/device/darmoshark.rs`：仅匹配已验证 N3 接收器及厂商集合；用官方只读命令核对接收器所配设备，再取得电量。不完整发现、休眠及通信失败保留缓存，确认配对身份变化后隐藏旧设备。没有 unit identity，不能区分同接收器重新配对的另一台同型号 N3。
- `src/device/compx.rs` / `razer.rs`：只读协议与明确profile，分别核对FF02:0002的17字节厂商集合和MI00的91字节Feature集合；前者读取协议电源标记，后者独立查询充电。具体ID及来源见[设备支持表](DEVICE-SUPPORT.md)。
- `src/device/rog_pelta.rs`、`razer_headset.rs`、`aula.rs`：分别为PELTA控制输出、BlackShark 2023专用报文和SONiX设备信息；复用vendor_hid的发现/事务桥，通过Adapter.kind返回耳机或键盘，默认不选中；共享OEM查询未确认时用unconfirmed_name保留通用名称，成功缓存不被覆盖。AULA校验真实设备信息，耳机确认连接，充电不确定保持Unknown。
- `src/device/ajazz.rs`、`xiaomi.rs`：AJAZZ 2.4G网页驱动的分块设备信息和小米G-HUB M002特征报告；复用vendor_hid的物理身份与有界预算，仅访问已核对的厂商集合。AJAZZ校验返回的型号PID与2.4G工作模式后才认领键盘名称；小米接收器由VID/PID与0xFFEF集合选择，协议无型号字段，查询失败不改名。AJAZZ完整信息明确识别到其他目标时以NotFound停止查询/发现，隐藏旧逻辑设备但保留偏好；超时继续保留成功值。充电字段未确认，均保留Unknown。
- `src/device/delux.rs`：Beken专用000A:0000五字节只读状态集合；接收器ID+帧内型号/事件双重筛选，首次成功才认领。每次最多2.3秒/64帧，不新增监听线程。休眠不清零，按物理接收器、PID和帧标识隔离；不读普通鼠标输入、不冻结充电读数。
- `src/device/vendor_hid.rs`：已配置厂商适配器共用的有界发现、物理身份、候选失败隔离和原生传输桥；每个端点事务共享1.6秒预算，最多4个符合条件的控制集合，不打开普通输入流。
- `src/device/hid_io.rs`：共享原生重叠Feature、Output和指定长度vendor Input；预挂Input后再控制输出/WriteFile的exchange采用Flight持有每个已提交IRP，任意退出均取消、等待或隔离；新增GENERIC_READ专用入口供DELUX使用，未改变原Feature零访问与其他读写入口；N3事务共享1.5秒预算，取消清理最多额外等100毫秒。极端驱动取消未完成时，最多保留 8 份请求及其句柄、内存与接收器锁，在原线程确认完成后释放；未完成端点拒绝新查询。不新增清理线程，不读取正常键鼠输入。
- `src/manager.rs`：按身份独立的监控/托盘/提醒选择，成功缓存及v6本地状态；向后读取v4/v5。读取返回Result，只有NotFound可用默认值；访问失败、无效UTF-8及未知文件头停止启动，既有合法版本内的逐行容错保持。
- `src/notifications.rs`：品牌无关的提醒周期与Windows提交。新有效读数推进周期，保存成功后投递；失败保留待投递项。
- `src/app.rs`：主线程消息循环、三页控件、后台任务/结果桥、Shell菜单。同步回调也显式唤醒循环。
- `src/icons.rs`：按类型和电量生成当前HICON，主窗口与托盘使用同源填色几何。
- `src/save_dialog.rs`：同EXE短时保存窗口进程，动态加载系统comdlg32；通过有界UTF-8管道返回路径，临时小栈读取线程随后join。保存进程另持所属主进程的同步句柄，所属进程退出即结束自身；该监视线程只存在于临时进程。取消或退出清理自有子进程，不启用监控、迁移或自启，不写诊断文件；常驻进程不保留文件窗口Shell缓存。
- `src/launch.rs`：前台/后台启动模式及可见启动错误，CLI保持JSON错误；`src/startup.rs`：真实自启与后台参数快捷方式，兼容升级旧参数且不改禁用状态；`src/licenses.rs`：单EXE许可文本。

## 状态与调度

监控选择和连接状态分开。休眠/超时保留成功值与成功时间；确认断开后隐藏显示，选择和缓存仍在，重连恢复。启动尚未确认连接的缓存可先显示。只有无成功值时才用未知。

HID++先探测直连地址FF，成功则不再重复查询其别名槽位，否则查接收器1–6；名称、类型、电量功能仍动态取得。

有百分比就使用设备值；纯档位由适配器解释编号和范围后归一化。G304的90来自设备，固件仍按离散档位变化；UI十格不代表测量精度。

读取串行、重复刷新合并、低频定时兜底；设备变化、恢复及可用协议事件请求有界刷新。UI不等待HID或GATT。MTA与WinRT会话同寿命，UI的COM引用覆盖消息循环。Native资源成对释放，控件缓冲仅存在于一次绘制。

## 蓝牙边界

仅访问已配对、当前连接且已有访问许可的BLE设备。读取180F中的2A19；Notify/Indicate可用则订阅，否则低频读。可选2BED按标准解释充电，缺失或失败保持未知，不猜私有字段。先取基本电量，再用余量读可选状态。发现预算8秒，主动读取预算3秒，取消与失败不会卡住UI。

单设备/服务失败保留成功候选及未完整确认设备的旧值。AEP请求GUID类型的 `System.Devices.Aep.ContainerId`，与HID使用相同字节序容器键；缺失时保守回退。会话也核对Windows配对身份。属性定义见 [Microsoft文档](https://learn.microsoft.com/en-us/windows/apps/develop/devices-sensors/device-information-properties)。

## 分发与隐私

运行不需要Rust、Python、SDK或驱动安装。所有正常运行实例的数据统一放在 Windows Known Folder LocalAppData下的 `QuietBattery/devices-v2.state`；测试使用项目 `.build/tmp/`，构建不读取或迁移真实设置。`src/storage.rs` 在单实例锁内迁移旧 `data/` 或开发目录 `.build/` 的状态；先写完整临时文件并原子发布、逐字节核对后才清理旧状态。新状态已存在时不覆盖，冲突旧文件保留，清理不递归。迁移发布使用不带替换标志的MoveFileExW，检查之后并发出现的目标也不得覆盖。迁移失败保留旧数据并返回错误；不能悄悄回退到EXE旁。读取设置在创建窗口、worker及升级快捷方式之前完成。

`src/status_ipc.rs` 使用临时隐藏消息窗口请求状态，托盘在请求时序列化内存模型并以WM_COPYDATA回复；窗口/PID/协议标识核对，回复最多1MiB，查询最多3秒，向客户端发送最多等待200毫秒。支持原生菜单嵌套消息循环，无常驻线程或状态文件。状态查询不触发硬件读取；`--refresh` 单独请求刷新。窗口创建和类注册在成功、失败、超时路径均清理。`--diagnose` 不初始化 HID++ / BAS；界面复制/导出通过现有串行worker按需收集所选设备，`--scan`仍附后台元数据缓存；不在主线程增加发现I/O。诊断只覆盖 HID 接口，不能代表所有 USB / PnP / BLE 设备。程序没有网络或遥测路径；设备身份与错误诊断留在本机，公开仓库只放源码、资产及核对过的截图。

GUI适配诊断不运行Registry发现/厂商查询，不更新Manager读数、成功时间或提醒周期。HID重新被动枚举后按物理身份筛选；BAS只对同一物理身份、已有许可且已连接的BLE端点读取标准特征，不新建订阅或变更配对。取消通过每次请求的token传递；页面关闭/切换后迟到结果不得覆盖剪贴板。导出临时文件留在用户选定目录，成功原子替换，普通失败/取消清理；不生成周期快照或日志。强制结束导出进程会绕过清理，发布前旧报告不变，但所选目录可能留下一个自有.tmp；不自动扫描/删除用户目录中的疑似临时文件。
