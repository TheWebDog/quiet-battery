# 设备支持与协议依据

本表包含v0.6.0候选适配，稳定版仍为v0.5.0，区分**本项目真机通过**和**已实现、待真机确认**。后者已内置发现、查询、解析和测试，可直接下载尝试，不是仅登记型号；但公开资料和模拟事务不能证明特定固件、接收器或Windows驱动一定可用。

下文VID/PID和报文字节均用十六进制，偏移从0开始；百分比仍用十进制。

## 已有真机证据

| 设备 | 连接与范围 | 未完成验证 |
|---|---|---|
| Logitech G304 | 046D:C53F接收器；HID++ 4.2 / 1000，曾真实取得90% | 其他接收器/型号、真实充电、实体拔插等不能沿用结论 |
| Darmoshark N3 | 248A:FF30接收器配对248A:FF10，历史真实9% | 非零充电字段含义尚未实现；同型号换配对身份限制 |
| AJAZZ AK820 MAX | 0C45:FEFC 2.4G接收器，FF80:0001 33字节设备信息，贡献者原版32–33%、审查修正版80%（2026-10-30） | 充电含义、休眠唤醒、拔插、厂商页面并发、其他AJAZZ型号 |
| 小米电竞鼠标 2 | 2717:5111 2.4G接收器，FFEF:0000 64字节Feature，贡献者原版27%、审查修正版20%（2026-10-30） | 充电、有线0x5110、休眠唤醒、拔插、其他小米鼠标 |
| 广播名BT5.0 KB的BLE设备 | 标准180F / 2A19，历史真实100%；未确认零售型号 | 变化订阅、可选充电、断开重连 |

以上是各版本历史记录，不代表最新版已对全部场景重复实测；本次结果见[验证记录](VALIDATION.md)。

## 内置协议：已实现、待真机确认

### Logitech HID++ 家族

不按零售型号建立白名单。入口是VID046D、usage FF00:0001/0002；先探测直连地址FF，失败再查询接收器槽位1–6。设备名称、类型、身份和电量功能动态查询。只有1000或1004确实返回有效读数才认定支持；1004按能力区分百分比与档位。已补慢速充电状态4。

首批目标：**G305、PRO Wireless（GPW）、PRO X SUPERLIGHT（GPX）、G502 LIGHTSPEED、MX Master / MX Keys中提供上述功能的型号**。G304仍为已有实测基线；这些目标没有在本项目逐型号真机通过。USB、蓝牙或接收器必须实际暴露可用HID++接口；不能从系列名推定所有连接方式。仅提供电压、HID++1.x或其他电量特性的设备本次不新增估算支持。

### ATK / VXE / VGN：Compx 17字节家族

| 型号 | VID | 2.4G接收器PID | 有线PID |
|---|---|---|---|
| ATK F1 Ultimate | 373B | 1031 | 102E |
| ATK A9 Ultimate | 373B | 11D9 | 11B6 |
| VXE MAD R | 373B | 104D | 103F |
| VXE MAD R Major Plus | 373B | 1040 | 104C |
| VXE R1 Pro Max | 3554 | F58A | F58C |
| VXE R1 SE+ | 3554 | F58E | F58F |
| VGN F1 Pro | 3554 | F503 | F502 |
| VGN F2 Pro Max | 3554 | FB3E | FB3D |

必须同时符合vendor集合FF02:0002、Input/Output长度17。仅识别厂商号、型号文字或系列名不会发送查询。型号其他后缀、替换接收器、1K/4K/8K版本不能自动套用。ATK Zero的64字节协议不在本次范围。

只读事务：report08 / command04，固定请求 `08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49`。回复须同report/command、长度17、全包求和模256为0x55；百分比在字节6，电源标记在字节7（0放电、1充电、其他未知）。拒绝请求回显、超范围百分比及损坏包。查询前有界排空该vendor输入队列，不读取普通鼠标/键盘输入。

公开来源的35%报文已加入解析测试，其他边界与事务是合成夹具。协议没有逐次请求序号：即使排空旧包、应用内互斥和校验，也无法完全协调厂商软件。

### Razer：90字节电源协议

所有VID为1532；下表仅覆盖这些明确PID。

| 型号 | 2.4G接收器PID | 有线PID |
|---|---|---|
| Viper V2 Pro | 00A6 | 00A5 |
| Viper V3 Pro | 00C1 | 00C0 |
| DeathAdder V3 Pro | 00B7 / 00C3 | 00B6 / 00C2 |
| Basilisk V3 Pro | 00AB | 00AA |
| DeathAdder V2 X HyperSpeed | 009C | 无；AA/AAA可更换电池 |

只访问USB接口MI00且Feature长度91（含Windows report-ID槽位）的控制集合。以零访问权限句柄提交Feature，不打开输入流；未满足入口规则保留不可用信息。不同描述符版本若不符合，需补实证，不能扫描任意接口试命令。

90字节payload：transaction=1F，class=07；80读电量、84读充电，data-size=02，CRC为payload字节2–87异或。Windows缓冲前加report-ID零；接收接受91字节，或仅缺末尾保留字节的90字节，二者都必须保留零report-ID和完整CRC。检查长度、事务、分包/协议字段、class/command、CRC和成功状态；回显、busy、外来命令继续有界等待，超时/失败不生成电量。电量原值0–255按最近整数换算到0–100%，有效零值不会被猜成休眠。009C只读电量，不发送充电命令；其他型号充电独立读取，失败只将充电状态置未知，保留已经成功的电量。事务值由协议固定为1F，校验无法完全排除其他软件同命令并发覆盖；需要真机补测。

共享HyperPolling接收器00B3、其他鼠标、键盘、蓝牙私有协议没有据此加入；共享接收器还需要可靠确认配对型号。

### v0.5.0：ROG PELTA、雷蛇耳机与AULA键盘

以下为独立实现的三种协议；与新增009C一起覆盖Issues中的四款设备。**本机没有这些设备，新版等待对应硬件回测。** Issue中的真实读数属于贡献者提供的协议/旧版运行证据，不是本项目新版实测。此节偏移及长度用十进制，报文与USB标识用十六进制。

| 设备 | 唯一允许的查询入口 | 读取及确认 |
|---|---|---|
| ROG PELTA | 0B05:1B84、MI03、FF00:0001、Input/Output均64字节 | 64字节`CC 12 07 …`回复byte6为0–100%；`CC 12 08 …`回复byte5为0未充电/1充电，其他值未知 |
| BlackShark V2 Pro (2023) | 1532:0555、MI03、FF00:0001、Input/Output均64字节 | `02 80 08 00 00 50 41 08 00 03 <cmd> …`；20查连接，21查电量，2A查充电；匹配报告号、PI头、序列/数据头、命令和成功/长度字段后取byte15 |
| AULA MINI 60 HE PRO | 0C45:FEFE、FF60:0061、Input/Output均33字节 | 零report-ID+32字节包；仅GET_DEVICE_INFO(10)，按地址0/24/48读24/24/8字节；匹配55魔数、命令、分块长度和地址；设备信息VID=0C45、PID=80A2/80B2且workMode=2才认领，byte17为百分比 |

PELTA以Issue #1的**后续更正记录**为准：必须先提交ReadFile挂起读取，再以IOCTL_HID_SET_OUTPUT_REPORT（HidD_SetOutputReport对应的控制输出）发包，不能用WriteFile。早先80/21猜测及FF07/FF0B通道不使用。回复必须分清07电量与08充电，避免迟到的另一个命令被解析成0%。1B86经典蓝牙未纳入标准BAS，也不推及其他ROG型号。

雷蛇耳机接收器在耳机关机时仍可枚举，故在电量读取前后查询连接；0为未确认连接，不产生新读数，保留缓存。基本电量确认后再用剩余预算读取可选充电，失败仅置充电未知，不丢弃电量。0555不能扩大为旧0528或Kraken；同命令厂商软件并发仍需真机检查。

AULA共享接收器ID不能证明型号，必须校验返回的设备VID/PID与2.4G工作模式；不支持就保留未适配候选。充电byte18的含义未确认，原值保留Unknown，不把0/1猜作充电；有线80A2/80B2通道未接入。原始完整分块头、空闲后恢复、关机时接收器是否返回旧信息仍须硬件验证；Issue #2附件记录过接收器空闲不应答，超时保留上次成功值。

三个适配器共用vendor_hid的物理容器/路由分组、失败隔离与1.6秒端点预算。每条查询最多两次收发，每次最多500毫秒且受总预算限制；先清理最多8份已完成旧输入，再挂起读取后发送已知查询。只有PELTA使用控制输出，另两个使用WriteFile。所有退出路径由RAII取消并等待完成，100毫秒后仍未完成则保留到现有最多8份的隔离队列；保有句柄、缓冲、OVERLAPPED和接收器锁，不增加常驻线程。只读专用vendor集合，不读普通键鼠输入、不设置设备。

资料：

- [Issue #1更正及实际回复](https://github.com/TheWebDog/quiet-battery/issues/1#issuecomment-6295618334)：U98de提供PELTA电量68/69/70/71/74及充电帧；提交补丁未编译，本次没有直接导入其原生I/O。
- [Issue #2与附件](https://github.com/TheWebDog/quiet-battery/issues/2)：askyfullofflower提供的附件文档报告009C读到10%、0555读到90%、AULA固件1.55读到56%；包含Synapse的95%原始耳机回复及AULA设备信息样本。附件基于0.3.2，未整包覆盖，保留既有DELUX适配。
- [Ferris Pulse耳机协议](https://github.com/Ferris-echo/FerrisPulse/blob/b1a730bfa783f5833cda7435b6e7d5a2f2d4961f/src/Hardware/RazerHeadsetHelper.cs)：64字节报文及连接/充电命令。仅查协议事实，不复制GPL实现或其设备控制功能。
- [OpenAula协议](https://github.com/cvanelteren/OpenAula/blob/fc3a9200bffd62ac91ca3bf5b3aac56a906a530a/docs/PROTOCOL.md)：SDK重建的报文头、分块地址、设备信息布局和只读命令。
- [Microsoft控制输出](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/hidclass/ni-hidclass-ioctl_hid_set_output_report)：输入缓冲含report-ID；完成时不要求输出字节数。

### v0.6.0候选：AJAZZ AK820 MAX 与小米电竞鼠标 2（贡献者报告）

两款来自[PR #3](https://github.com/TheWebDog/quiet-battery/pull/3)贡献者导出的 `selected-device-support` 报告，协议依据为各自官方网页驱动。驱动包只做静态 JS 核对，未在浏览器中运行，也未导入其代码或网页运行时。下表偏移从0开始，报文和USB标识用十六进制。

**AJAZZ AK820 MAX**

- 接收器 `0C45:FEFC`，产品串 `AJAZZ AK820MAX`；只访问MI02的 `FF80:0001` 集合，Input/Output均33字节（含Windows report-ID槽位），report ID 0。
- 只读 `GET_DEVICE_INFO(0x10)`：零report-ID后的32字节包为 `AA 10 18 <addr LE16> 00 <last> 00`，按地址0和24各取24字节；回复须以 `55 10 18 <addr LE16>` 开头，Windows report-ID必须为0，声明长度必须24，分块地址严格匹配；地址0不能代表缺失地址。贡献者2026-10-30在审查修正版回测确认硬件第二块确实回显24，两块原始头已作为脱敏夹具。
- 设备信息：byte4/5 为设备VID、byte6/7 为设备PID、byte16 为工作模式、byte17 为百分比、byte18 为充电原值。只有VID=0C45、PID=80A0且工作模式=2才认领AK820 MAX；贡献者原版32–33%、审查修正版80%。
- 键盘休眠时接收器仍枚举，并可能用 `55 FC 04` 报告2.4G断开；此时取不到设备信息，不产生新读数，已有成功值按缓存规则保留；首次失败保留通用名称，已有成功值遇超时继续保留；完整有效设备信息明确返回其他VID/PID或非2.4G模式时，旧逻辑设备隐藏但保留偏好/缓存。相同型号换配对仍缺独立unit ID。充电byte18含义未确认，保留Unknown，不显示闪电。
- 未覆盖：其他AJAZZ型号/接收器、有线或蓝牙通道、充电、实体休眠唤醒、拔插和厂商页面并发。

**小米电竞鼠标 2**

- 接收器 `2717:5111`，产品串 `Mi Gaming Mouse 2 Receiver`；只访问MI03的 `FFEF:0000` 集合，Input/Output/Feature均64字节（含report-ID槽位）。
- 只读电量：Feature report 5，载荷首字节为选择符1（0x5111接收器）、第二字节 `16`；等待约100ms后读取Feature report 5。贡献者原版回复为 `05 01 16 03 01 1B …`（Windows缓冲含report ID），byte5为百分比；原版27%、审查修正版20%。
- 官方G-HUB的M002路径使用同一Feature查询；0x5110有线/直连变体使用选择符0，本次未接入，不能只按相同命令扩大。电量回复还要求byte3为已记录的3字节数据长度，拒绝请求回显/空数据；不将byte4硬猜为成功或充电标志。充电未确认，返回Unknown(255)。
- 未覆盖：有线0x5110、标准报告路径、充电、实体休眠唤醒、拔插和其他小米鼠标。

资料：

- [AJAZZ网页驱动](https://ajazz.driveall.cn/)：`3141-65276-AJAZZ AK820MAX` 配置、命令16和24字节分块。2026-10-28静态核对。
- [小米G-HUB Beta](https://web.mieco.net/yuemi--manual/mouse/h/p/index.html)：设备筛选 `10007/20753/0xFFEF/usage 0`、M002 Feature 5与选择符1。2026-10-29静态核对。

维护者2026-10-29仅静态复核以下固定资源，未运行网页、安装厂商程序或提交其源码：

- AJAZZ `layout-classic-CMxiG14P.js`，SHA256=`6c02a3e2a333b70f0fe75ec5bfaa68dec2a242c1e6e801d1c545ff6f7759fa5a`；`GX`设备信息48字节、`NX/PX/FX`分块、报告0和24字节有效负载；SDK默认不强制地址校验，严格地址兼容已由贡献者2026-10-30两块真实回复确认。[资源](https://ajazz.driveall.cn/assets/layout-classic-CMxiG14P.js)
- 小米 `index-BbzxWOJP.js`，SHA256=`a93a4b1af62c120faa9a64b1cd67bbae3b6d723b36d5acf830f5bc6e210473f6`；`W3/G3/Sf`确认选择符1、Feature 5、100ms及byte5电量；`processStandardReport`取4–6的三字节数据。`03 01`中byte4的完整语义未在本轮确认，不据此实现充电。[资源](https://cdn.mieco.net/yuemi--manual/mouse/static/assets/index-BbzxWOJP.js)

贡献者已于2026-10-30在审查修正版回测两款并提供脱敏原始回复夹具：AJAZZ两块头 `00 55 10 18`、第二块 `addr=24/last=1`，小米 `05 01 16 03 01 14 15…`（20%）。充电、断电/休眠/唤醒、未就绪回复与0%实机边界、同类多台仍待回测；维护者本机仍无这两款硬件。

### 多彩 DELUX：Beken五字节主动上报

v0.4.0新增，**以下均为已实现、待本项目真机确认**。不是17款鼠标，而是17组帧内型号/事件组合。同名或相似外观不能证明协议相同。

仅访问接收器`1D57:FA60`或`1D57:FA65`的`usage page=000A / usage=0000`集合：Input长度恰为5，Output/Feature长度均为0。以GENERIC_READ打开，不发Feature、Output、解锁、唤醒或设置命令；不读普通键鼠输入。FA60被其他OEM共用，必须收到下表匹配帧才认领；`10`（M600/R1歧义）和`55`（X11）不按共享ID认领为多彩。

| 系列/驱动显示名 | 帧内型号码 | 电量事件 |
|---|---|---|
| M600 Pro / M600 Pro 8K | 20 / 21 | 40 |
| M900 Pro（含不同状态帧变体） | 30 / 3F / 50 | 30、3F用40；50用41 |
| M900 | 31 | 40 |
| M900 Mini | 3D | 40 |
| M800 Mini（驱动内部三组变体） | 29 / 2A / 2B | 40 |
| M800 Pro 3395 / 3311 / Pro+ / 3370（Beken变体） | 32 / 33 / 34 / 35 | 40 |
| M500 Pro / M300 Pro / M700 Pro | 37 / 38 / 39 | 40 |

Windows输入缓冲为`03 <型号> <事件> <电源标志> <百分比>`，长度严格5。电源1=放电、2=满电、3=充电，其他值未知；百分比接受0–100，拒绝越界。不因100%伪造满电状态，不把充电电量冻结/平滑，也不推算续航。固件充电时若上报跳动，软件仍显示原值。M900 Mini命名由独立型号表交叉确认；官网部分资源只写M900，不能据此保证具体Mini Pro/Ultra零售后缀。

每个物理接收器/路由一次最多监听2.3秒、64帧、4个合格集合；发现整体最多8组/12秒。没有上报只是不确定，保留已有成功值；清单不完整或身份字段暂不可用也不据此判离线。首次无有效帧时仍留通用未适配设备，不能生成虚假电量。新的有效型号使用独立身份；同型号重新配对另一台鼠标仍缺独立unit ID。有线与接收器不按同名合并。

**尚未覆盖：**旧`248A:5B2E/5B2F` M800 Pro 3395的独立Feature协议、其他主控的Ultra/Pro+、M618等办公系列私有2.4G通道、未知/替换接收器和上述有线PID。已找到旧M800的查询线索，但Windows集合入口和完整回复契约不足，不套用Beken规则。标准蓝牙BAS仍按现有共用实现处理，不代表多彩蓝牙已逐型号实测。

协议依据：官网[20251112驱动包](https://www.deluxworld.com/uploads/admin/file/20260108/DELUX%20Gaming%20Driver%2020251112.rar)，从[下载页](https://www.deluxworld.com/service.html)取得，**仅离线解包和静态分析，未运行/安装驱动**。包SHA256=`565a53adaa38e6793a09881e3ec51fe0b8d1ef7dff828e29d667863c5beb621a`；解包后DELUX.exe SHA256=`2d1f3770150adca7772908e06ff41ab8e0de8ab95e477d200ce00d01e664715f`。按接收回调逐项追到17组型号处理函数，核对事件、百分比高字节和2/3电源分支；例如M900 Pro的处理入口VA004D82E0，50变体VA004A0A40，后者事件比较VA004A0B07确为4100。只把线上的字段契约独立实现进Rust，未引入厂商程序、DLL或第三方驱动源码。

另交叉核对[M900 Pro原始状态记录](https://github.com/victornoleto/keyboard-mouse-config/blob/291c6da6a76b8293d00a4ce9c9c49e2b090605c7/devices/delux_m900pro/PROTOCOL.md)、[M900 Pro 30/40贡献者样本](https://github.com/evren-oct/vigil-watt/blob/d434f90f6be61a28a0dc91c693c410e08964fac4/docs/hardware/delux.md)、[M600 Pro抓包及Windows集合说明](https://github.com/OpenMouse-Project/mouse-protocol/blob/bac031567b28f2a55c616fc525b03f2d5fa452cc/docs/delux-m600-pro-testing.md)、[型号码表](https://github.com/HarukaYamamoto0/attack-shark-x11-driver/blob/e4d68c0a191bbbf5ac6cf3919d5fb9bdf0823a0f/src/core/devices.ts)。社区最初对50/41百分比的判断是暂定，本次再与官网4100电量分支交叉确认；这些仍不是本项目的M900 Pro真机结果。原始包、定点反汇编、独立核对脚本及记录留本地validation，不进入发布附件。

## 来源与实现边界

这些链接用于核对协议事实、型号标识和报文，未将第三方驱动作为运行依赖，也未导入其源码。Rust适配器按上述协议契约独立编写。第三方项目中的“支持”或真机记录不改写成本项目的实测结论。

- [Logitech官方HID++资料](https://github.com/Logitech/cpg-docs)：基础报文与功能模型。另对照[Solaar HID++实现](https://github.com/pwr-Solaar/Solaar/blob/e7304c4c451cc9bb4f206a914844525e67856a28/lib/logitech_receiver/hidpp20.py)的电量状态定义。
- [mouse_tray Nordic52协议及型号表](https://github.com/Fan4Metal/mouse_tray/blob/5dfb40d0fcbf98b8d14aa2ba47a7ed6768c466d7/mouse_tray/drivers/chipset/nordic52.py)：17字节请求、35%公开回复及8组ID。
- [OpenRazer报文结构](https://github.com/openrazer/openrazer/blob/477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d/driver/razercommon.h)、[电源命令](https://github.com/openrazer/openrazer/blob/477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d/driver/razerchromacommon.c)、[接口和事务规则](https://github.com/openrazer/openrazer/blob/477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d/driver/razermouse_driver.c)、[型号ID](https://github.com/openrazer/openrazer/blob/477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d/driver/razermouse_driver.h)。与部分简化工具不同，不交换data-size和command-class位置。

## 没有硬件时的验收范围

1. 固定来源版本，逐字段核对查询只读性、接口、长度、请求和回复；对未知协议不试发枚举命令。
2. 测试实际查询函数的模拟完整事务，覆盖回显、无关回复、busy、错误、超时/次数上限、充电失败和取消；不是只测试一个孤立的字节解析函数。
3. 测试所有profile的VID/PID及控制集合筛选、0/100与越界、报文截断、损坏校验、物理身份和协议路由隔离。使用原生命名管道核验底层收发及取消所有权。
4. 跑fmt、单测、Clippy和release，使用可用的G304回归，短时检查进程和资源；下载发布成品再核对哈希和版本。
5. 这些检查足以发布**明确标注待真机确认的内置适配**，不等于新型号真机认证。后续实际读数、固件/接收器差异、充电、休眠/唤醒、拔插、厂商驱动并发与多台设备仍需独立记录。

Compx/Razer、PELTA及AULA身份由物理容器（缺失时按接口路径回退）、VID/PID与有线/接收器路由组成；未读取独立配对unit ID，同接收器重配同型号可能沿用旧缓存。有线与接收器未证明属于同一台时保持分开，可能出现两条记录；不按同名合并，避免误吞同型号的另一台设备。多槽位接收器的实体休眠与换配对场景仍待测。

## 后续接手

新型号先按[扩展指南](ADDING_DEVICES.md)导出匿名诊断。协议相同就补profile和来源；差异则补规则与事务夹具，全新协议才加模块。先核对集合长度和接口再改筛选条件，不为“能枚举”放宽到向所有HID发送命令。取得真机后记录构建哈希、确切型号/接收器、连接方式、连续读数及失败恢复，才升级相应型号和场景的验证状态。
