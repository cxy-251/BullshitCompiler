use bsc_opt::PassDiffRecord;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct WasmCompileResponse {
    pub success: bool,
    pub error: Option<String>,
    pub ast_json: Option<String>,
    pub initial_ir_dump: Option<String>,
    pub pass_diffs: Vec<PassDiffRecord>,
    pub optimized_ir_dump: Option<String>,
    pub emitted_code: Option<String>,
    pub logs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WasmDecompileResponse {
    pub success: bool,
    pub aspects: Vec<String>,
    pub brutal_truth: String,
}

pub fn compile_internal(source: &str, target: &str) -> WasmCompileResponse {
    let mut logs = Vec::new();
    logs.push(format!("[STAGE 0: DOC-INIT] 启动长文本/文档编译流水线, 接收字符数: {}, 目标方言: {}", source.len(), target));

    let (ast, mut ir) = match bsc_frontend::compile_source_to_ir(source, "wasm_session") {
        Ok(res) => {
            logs.push(format!("[STAGE 1: DOC-FRONTEND] 自然语言词法与语法分析完成: 切分构建 {} 个语句/事件节点 (AST)", res.0.statements.len()));
            logs.push(format!("[STAGE 2: INTENT-IR] 初始意图中间表示生成: 分配 {} 个全局意图符号, 创建 entry 基本块", res.1.values.len()));
            res
        }
        Err(e) => {
            logs.push(format!("[STAGE 1: FRONTEND ERROR] 语法解析异常: {}", e));
            return WasmCompileResponse {
                success: false,
                error: Some(e.to_string()),
                ast_json: None,
                initial_ir_dump: None,
                pass_diffs: Vec::new(),
                optimized_ir_dump: None,
                emitted_code: None,
                logs,
            };
        }
    };

    let ast_json = serde_json::to_string_pretty(&ast).ok();
    let initial_ir_dump = Some(ir.format_dump());

    logs.push("[STAGE 3: OPT-PIPELINE] 启动语义中端 Pass 优化流水线:".to_string());
    let pass_diffs = bsc_opt::optimize_module(&mut ir);
    for (idx, diff) in pass_diffs.iter().enumerate() {
        logs.push(format!("  [Pass {}] {} - {} (重写生效: {})", idx + 1, diff.pass_name, diff.description, diff.modified));
    }
    let optimized_ir_dump = Some(ir.format_dump());

    logs.push(format!("[STAGE 4: BACKEND-CODEGEN] 启动后端目标方言发射器 (Target: {})...", target));
    let emitted_code = match bsc_backend::emit_target(target, &ir) {
        Ok(s) => {
            logs.push(format!("[STAGE 4: BACKEND-CODEGEN] 目标方言结构化公文生成完毕: 产出 {} 字符", s.len()));
            Some(s)
        }
        Err(e) => {
            logs.push(format!("[STAGE 4: CODEGEN ERROR] 后端发射异常: {}", e));
            Some(format!("Error: {}", e))
        }
    };
    logs.push("[STAGE 5: COMPLETE] 全文本文件编译流水线执行完成 ✓".to_string());

    WasmCompileResponse {
        success: true,
        error: None,
        ast_json,
        initial_ir_dump,
        pass_diffs,
        optimized_ir_dump,
        emitted_code,
        logs,
    }
}

pub fn decompile_internal(jargon: &str) -> WasmDecompileResponse {
    let decompiler = bsc_decompiler::Decompiler::new();
    let res = decompiler.decompile(jargon);
    WasmDecompileResponse {
        success: true,
        aspects: res.extracted_key_aspects,
        brutal_truth: res.brutal_truth,
    }
}

// ---------------------------------------------------------------------------
// Standard WebAssembly C-ABI Export (Zero JS-glue dependency)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn bsc_alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn bsc_dealloc(ptr: *mut u8, size: usize) {
    if !ptr.is_null() && size > 0 {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

/// Compiles source text to SSA IR & Target Dialect Jargon.
/// Returns pointer to: [4-byte little-endian length][UTF-8 JSON bytes].
#[no_mangle]
pub unsafe extern "C" fn bsc_compile_wasm(
    source_ptr: *const u8,
    source_len: usize,
    target_ptr: *const u8,
    target_len: usize,
) -> *const u8 {
    let source_slice = if source_len > 0 && !source_ptr.is_null() {
        std::slice::from_raw_parts(source_ptr, source_len)
    } else {
        &[]
    };
    let source = std::str::from_utf8(source_slice).unwrap_or("");

    let target_slice = if target_len > 0 && !target_ptr.is_null() {
        std::slice::from_raw_parts(target_ptr, target_len)
    } else {
        &[]
    };
    let target = std::str::from_utf8(target_slice).unwrap_or("alibaba");

    let response = compile_internal(source, target);
    let json_bytes = serde_json::to_vec(&response).unwrap_or_default();

    let mut out = Vec::with_capacity(4 + json_bytes.len());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&json_bytes);

    let ptr = out.as_ptr();
    std::mem::forget(out);
    ptr
}

/// Decompiles bullshit jargon to brutal truth.
#[no_mangle]
pub unsafe extern "C" fn bsc_decompile_wasm(
    jargon_ptr: *const u8,
    jargon_len: usize,
) -> *const u8 {
    let slice = if jargon_len > 0 && !jargon_ptr.is_null() {
        std::slice::from_raw_parts(jargon_ptr, jargon_len)
    } else {
        &[]
    };
    let jargon = std::str::from_utf8(slice).unwrap_or("");

    let response = decompile_internal(jargon);
    let json_bytes = serde_json::to_vec(&response).unwrap_or_default();

    let mut out = Vec::with_capacity(4 + json_bytes.len());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&json_bytes);

    let ptr = out.as_ptr();
    std::mem::forget(out);
    ptr
}

/// Compiles and executes a classic programming language source file (defaults to -O2).
#[no_mangle]
pub unsafe extern "C" fn bsc_compile_classic_wasm(
    source_ptr: *const u8,
    source_len: usize,
) -> *const u8 {
    bsc_compile_classic_wasm_opt(source_ptr, source_len, 2)
}

/// Compiles and executes a classic programming language source file with explicit opt level (0=O0, 1=O1, 2=O2).
#[no_mangle]
pub unsafe extern "C" fn bsc_compile_classic_wasm_opt(
    source_ptr: *const u8,
    source_len: usize,
    opt_level_num: u32,
) -> *const u8 {
    let source_slice = if source_len > 0 && !source_ptr.is_null() {
        std::slice::from_raw_parts(source_ptr, source_len)
    } else {
        &[]
    };
    let source = std::str::from_utf8(source_slice).unwrap_or("");
    let opt_level = match opt_level_num {
        0 => bsc_classic::OptLevel::O0,
        1 => bsc_classic::OptLevel::O1,
        _ => bsc_classic::OptLevel::O2,
    };

    let response = bsc_classic::compile_classic_pipeline_with_opt(source, opt_level);
    let json_bytes = serde_json::to_vec(&response).unwrap_or_default();

    let mut out = Vec::with_capacity(4 + json_bytes.len());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&json_bytes);

    let ptr = out.as_ptr();
    std::mem::forget(out);
    ptr
}
