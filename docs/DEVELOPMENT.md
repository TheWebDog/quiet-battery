# 构建、诊断与发布

## 构建

Windows x64、Rust stable、MSVC工具链与Windows SDK。维护者当前验证工具链为Rust1.94.0；PR #3贡献者报告使用1.99.1。Cargo.lock锁定依赖，缓存留在 `.build/`。Windows文件/产品资源及manifest版本从Cargo包版本生成，升级不另改硬编码版本号。

```powershell
./tools/build.ps1                # fmt、test、Clippy、release，复制到dist
./tools/build.ps1 -SkipCopy      # 只暂存，不替换运行实例
./tools/build.ps1 -Offline       # 缓存完整时可离线
```

运行中的EXE先正常退出再替换；不要结束鼠标驱动或改变自启偏好。发布由维护者选择通过验证的成品。

`tools/github-actions-build.yml.example` 提供执行相同检查的工作流模板。2026-10-20复核：仓库可写且账号为管理员，但CLI OAuth令牌仅有`gist/read:org/repo`，缺`workflow`，因此尚未启用。仓库写权限不能替代[OAuth workflow作用域](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps)。维护者可先执行`gh auth refresh -h github.com -s workflow`并完成浏览器授权，再将模板复制到 `.github/workflows/build.yml` 提交，等待实际Actions成功后才标记启用。

图标已提交，普通构建不需要Python。修改图标后用Python/Pillow执行 `tools/generate_assets.py`，同时核对原生显示。升级依赖或Rust后执行 `python tools/update_notices.py`；需要对应rust-docs，或同一编译器的已提交声明。

## 启动行为

无参数运行立即显示主窗口；再次运行恢复现有实例的窗口，不创建第二个常驻进程。`--background` 仅启动托盘，供登录自启或后台验证使用。自启快捷方式使用该参数；升级时只更新目标确为当前EXE、原参数为空的旧快捷方式，不创建新启动项、不更换目标、不清除Windows禁用状态，不改其他自定义参数。缺失或已禁用的自启不会被恢复。

正常启动失败显示原生错误弹窗和处理建议；CLI诊断失败仍仅输出JSON并返回非零退出码，不让脚本卡在弹窗。原生弹窗测试使用隔离桌面子进程，不切换用户输入桌面。快捷方式参数升级失败会提示，主程序继续启动；没有新增错误日志文件。

## 诊断

GUI子系统EXE应等待退出并捕获UTF-8输出；设备ID用参数列表传递，不拼接未经转义的Shell命令。

| 参数 | 用途 |
|---|---|
| `--scan` / `--probe <id>` / `--once` | 发现并查询支持协议、读取指定设备、单次兼容读取；scan 含通用集合缓存 |
| `--diagnose [--include-identifiers]` | 独立的被动 HID 清单，不初始化 HID++ / BAS，不查询电量 |
| `--status` / `--refresh` / `--quit` | 当前快照、后台刷新、正常退出 |
| `--startup-status` | 读取真实自启状态 |
| `--test-notification` | 隔离的原生通知自测，不改电量和提醒周期 |
| `--version` / `--licenses` | 版本与二进制内的完整许可 |

设置仅保存至 `%LOCALAPPDATA%\QuietBattery`；正常启动不在EXE目录创建文件。旧版 `data/devices-v2.state` 自动迁移，开发目录旧 `.build/devices-v2.state` 也能迁移；新设置已存在时不覆盖。只清理确认一致的旧状态、旧运行快照及空 `data` 目录，失败保留原文件。测试夹具使用 `.build/tmp`，不得在测试中改真实LocalAppData设置。

`--status` 从正在运行的同会话托盘按需取得JSON，不再读取或周期性写入 `runtime-status.json`，原有JSON字段保持兼容；未运行、旧版本不支持IPC或超时返回错误，不取过期文件充数。需要文件时由调用方明确重定向保存；截图与采样工具也通过该接口查询。`--diagnose` 本身仍只输出标准输出，只有显式运行导出工具才落盘。

`--simulate-device-error`、`--simulate-recovery`、`--simulate-explorer-restart`是验证钩子，不是物理休眠、拔插或系统重启证据。公开截图不附原始状态文件与身份。

界面复制/导出使用`selected-device-support`按需报告，详情见扩展指南；`--scan` 先运行一次发现，再序列化该次成功更新的缓存，发现失败时可能保留此前缓存。`--scan`附带的缓存不保证每次序列化都重新读取端点。需要当前机器的新证据时单独执行被动 `--diagnose`。

### 可重复的被动导出

在项目目录运行（发布 EXE 可用 `-Executable` 指定完整路径）：

```powershell
./tools/export-diagnostics.ps1 -OutputPath ./validation/diagnostics.json
# 仅在本机针对性开发需要定位端点时使用：
./tools/export-diagnostics.ps1 -IncludeIdentifiers -OutputPath ./validation/diagnostics-local.json
```

脚本重定向并等待 GUI 子系统进程，按 UTF-8 保存 JSON；20秒仍未退出则终止自己启动的诊断进程，不保存半份结果。`--diagnose` 本身也可由具备重定向和等待能力的进程 API 调用。不要仅凭进程启动成功断言有结果。

报告中的 `group-N` / `endpoint-N` 只在该份报告内有效，不是用户设备 ID。默认不输出接口路径、容器标识、序列号；错误只含操作和数字错误码。程序不调用序列号查询。显式 `--include-identifiers` 增加 `path` 和 `container_id`，路径可能含序列号或其他可识别本机的内容，只用于本地定位，分享前先核对。`--scan`仍包含现有设备身份，不能按匿名诊断报告处理；界面复制/导出默认隐藏这些标识。

每个 `fields` 项包含 `value`、`source`、`error`、`truncated`。数值按十进制字符串导出，缺失值为 `null`；不能把未知当作0。后台缓存中的 hidapi 零 VID/PID/release/usage 因无法区分实际零和底层失败回退，会标记为未知；原生查询成功返回的零仍保留为有效值。

Windows 总线 GUID 使用 `bus` 字段，hidapi 总线类型另存 `hidapi_bus_type`，两者独立保留来源。

`complete` 表示清单遍历完整，不代表每个元数据字段可访问；字段与端点错误保留其他证据。`complete` 只反映是否遍历到末尾：文字过长时可同时为 `complete=true`、`truncated=true`。最多512个集合、每段文字512字符、遍历预算8秒；超过数量或时间预算时 `complete=false` / `truncated=true`，正好512个且没有后续端点则完整。枚举中途失败时保留已有行并标记 `complete=false`，错误并不自动表示触发了裁剪。取消返回错误、丢弃本次报告；同步 Windows 调用不能保证即时取消或严格8秒退出。

Input / Output / Feature 长度来自零访问权限句柄的 `HidD_GetPreparsedData` / `HidP_GetCaps`。它们是 Windows 顶层集合的最大字节长度，包含 report-ID 槽位；不是电量包长、每个 report-ID 的布局或原始报告描述符。诊断不调用 `hidapi.open_path`，不设置输入缓冲，不读输入流，不发 Feature、Output、厂商写命令，不更改系统偏好，也不联网。

这是当前可枚举 HID 接口的清单，不是所有 USB、PnP 或已配对 BLE 设备的全集。没有 HID 接口的设备可先在设备管理器查看属性，或执行只读 `Get-PnpDevice -PresentOnly`，再用选定项的 `Get-PnpDeviceProperty -InstanceId <本机实例ID>` 检查 Hardware IDs、描述、厂商和容器关系。不要公开整机 PnP 身份列表。无 HID 接口也不证明不支持标准 BAS；该路径仍须按真实服务和访问许可验证。

## 适度核验

设备修改后读一次真机；常驻逻辑修改后确认单实例、正常退出、错误保留/恢复与菜单响应。界面改动看实际窗口和托盘。

```powershell
./tools/sample_running.ps1 -DurationSeconds 30 -SampleSeconds 3
```

采样记录EXE哈希、Working Set、Private Bytes、CPU及GDI/USER/句柄。默认30秒，异常再延长，不裁剪工作集。生命周期检查先确认没有用户运行实例，再对测试启动的实例做启动/正常退出；自启偏好保持不变。菜单与可见界面操作遵守当前用户的桌面授权。

## 发布

1. 核对代码、支持范围、许可、截图；执行完整构建及必要真机检查。
2. 从 `dist/quiet-battery.exe` 复制成品为 `quiet-battery-windows-x64.exe`，生成SHA256与简明说明。
3. 正常版本使用稳定 Release（不标 prerelease），标为 latest；说明G304、N3、标准蓝牙BAS及新增协议/型号的实际验证范围与未测场景，FL980保持搁置、未支持。发布后重新下载成品，核对 SHA256、`--version` 与 Windows 文件/产品资源版本。
4. 从v0.3.0起保留既有Release及附件，不再执行旧版清理规则；Git标签和提交历史同样保留。说明哪些设备有本项目真机证据、哪些仅有协议/夹具证据。
5. 不上传缓存、本地data、原始validation、旧设计和机器身份。许可与第三方声明已编入EXE，也可附文本供查阅。

## 适配报告维护

普通用户直接使用添加页的复制/导出；支持已监控和未适配设备。报告格式见ADDING_DEVICES，公开样例只使用合成字段，不提交本机原始报告。后台串行采集，与监控缓存分开，不修改提醒状态；完成消息仍唤醒主循环。报告2MiB、相关HID32集合、BLE候选32与电池服务4个有界；HID枚举预算8秒、标准BLE诊断预算5秒，同步Windows调用仍可能超出软预算。

保存窗口由同EXE内部参数`--support-save-dialog <owner-hwnd>`启动，属于临时实现接口，不是用户自启入口。独立STA只负责选路径，不采集设备、不读写设置，不经过单实例锁；取消/退出会结束子进程，管道读取线程随之join。实际报告由现有串行worker写到选定位置。测试须区分模拟取消/文件错误、真实保存窗口与BAS真机路径，并在导出后检查资源，避免将系统文件窗口缓存留在常驻进程。

## 无外设的异常回归

`src/fault_tests.rs`只编入测试二进制。完整build会执行：真实Windows ACL拒绝、独占文件锁、保存重试、迁移发布前/后强制终止及并发目标保护；保存窗口取消和所属进程强制退出在私有桌面完成。启动错误弹窗使用实际设置锁错误，代理不切换用户桌面或接管鼠标。

测试文件仅在`.build/tmp/fault-*`；ACL只施加到新建夹具并恢复。测试子进程入口与暂停点均为`cfg(test)`，发布EXE不接受数据目录覆盖或故障环境变量，不改真实LocalAppData、自启、外设或厂商驱动。进程终止不等于物理断电；隐藏桌面弹窗不等于用户交互桌面的故障演练。

普通导出失败/取消会清理自有临时文件；若在写报告时强制结束进程，可能在所选目录留下`.quiet-battery-<pid>-<sequence>.tmp`，发布前的旧报告不变。确认进程退出后可删除该文件；程序不为此记录所有导出路径或扫描清理用户目录。
