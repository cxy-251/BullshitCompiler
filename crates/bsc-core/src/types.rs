use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentKind {
    FirstPersonSingular, // 我
    FirstPersonPlural,   // 我们
    ThirdPerson,         // 他 / 他们
    Organization,        // 团队 / 部门
    ImplicitSystem,      // 隐式系统 (经过 Pass 去人性化后)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediumKind {
    Bluetooth,           // 蓝牙
    WiFi,                // WiFi / 无线
    SerialPort,          // 串口 / UART
    Ethernet,            // 网线 / 以太网
    HttpRest,            // HTTP / API
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    EmbeddedBoard,       // 开发板 / 单片机 / 芯片
    ServerCluster,       // 服务器 / 集群
    Database,            // 数据库
    TerminalClient,      // 电脑 / 手机 / 客户端
    PeripheralDevice,    // 读卡器 / 打印机 / 传感器
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PayloadKind {
    Image,               // 照片 / 图像
    Document,            // 文件 / 文本
    LogStream,           // 日志 / 流
    BinaryCode,          // 代码 / 固件
    Telemetry,           // 传感器数据 / 报文
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    Connect,             // 连上 / 接入
    Transfer,            // 传 / 发送 / 同步
    Process,             // 处理 / 计算 / 修改
    Restart,             // 重启 / 恢复
    Verify,              // 检查 / 确认
    Crash,               // 挂了 / 崩溃
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutcomeKind {
    SuccessNormal,       // 没问题 / 正常运行
    FailureError,        // 报错 / 失败
    Unknown,
}
