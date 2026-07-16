import type { CommenterProjectProfileDraft } from './commenterTypes';

export const DEFAULT_COMMENT_PROMPT_TEMPLATE = `---
description:
alwaysApply: true
enabled: true
updatedAt: 2026-01-28T02:02:05.694Z
provider:
---

# 中文注释规范（Code Comment Style）

为代码添加注释时：
- 使用中文进行详细解释（这是学习型 fork）
- 包含示例和使用说明
- 解释"为什么"而不只是"是什么"
- 记录参数、返回值和边界情况
- 引用相关文件和概念

**示例（来自现有代码）**：

\`\`\`go
// Needle 索引映射接口
// 提供从 NeedleId 到磁盘位置的映射关系
// 不同实现有不同的内存/性能权衡:
//   - Memory: 全内存，性能最好，但内存占用高
//   - LevelDB: 基于 LSM-tree，内存占用低，性能适中
type NeedleMapper interface {
    Get(key NeedleId) (element *NeedleValue, ok bool)
    Put(key NeedleId, offset Offset, size Size) error
    Delete(key NeedleId) error
}
\`\`\`

## 注释覆盖范围

**不仅要在函数/结构体头部添加注释，还必须在以下位置添加注释：**

1. **包级别注释**
   \`\`\`go
   // Package storage 实现了 SeaweedFS 的核心存储功能
   // 包含 Volume、Needle、索引等关键组件
   package storage
   \`\`\`

2. **结构体和接口**
   \`\`\`go
   // NeedleMapper 提供 Needle ID 到磁盘位置的映射
   // 是 SeaweedFS 索引系统的核心接口
   type NeedleMapper interface {
       // Get 根据 NeedleId 查找其在磁盘上的位置
       // 参数:
       //   - key: Needle 的唯一标识符
       // 返回:
       //   - element: Needle 的磁盘位置信息（offset + size）
       //   - ok: 是否找到该 Needle
       Get(key NeedleId) (element *NeedleValue, ok bool)
   }
   \`\`\`

3. **函数内部的重要步骤**
   \`\`\`go
   func startVolumeServer() {
       // 【阶段 1：配置验证和解析】
       // 解析存储目录配置，支持多目录格式: dir1,dir2,dir3
       locations := parseLocations(*volumeFolders)

       // 验证每个目录是否存在且可写
       for _, loc := range locations {
           // 检查目录权限，确保有读写权限
           if err := checkDirPermission(loc); err != nil {
               glog.Fatalf("目录 %s 权限检查失败: %v", loc, err)
           }
       }

       // 【阶段 2：创建服务组件】
       // 根据 -index 参数选择索引类型
       // memory: 全内存索引，速度最快但内存占用高
       // leveldb: LSM-tree 索引，平衡性能和内存
       indexType := chooseIndexType(*indexType)
   }
   \`\`\`

4. **重要变量和参数**
   \`\`\`go
   // volumeId 是卷的唯一标识符，32 位无符号整数
   // 取值范围：0 ~ 4,294,967,295
   volumeId := uint32(3)

   // fid 是文件 ID，格式：volumeId,fileKey[_cookie]
   // 例如：3,01e3b0756f 或 3,01e3b0756f_a1b2c3d4
   fid := fmt.Sprintf("%d,%x", volumeId, fileKey)
   \`\`\`

5. **关键函数调用**
   \`\`\`go
   // 从 Master 请求分配新的 Volume
   // 参数说明：
   //   - replication: 副本策略，如 "001" 表示同机架复制一次
   //   - collection: 集合名称，用于逻辑分组
   //   - dataCenter: 指定数据中心，为空则自动选择
   resp, err := operation.Assign(masterClient, &operation.VolumeAssignRequest{
       Replication: replication,
       Collection:  collection,
       DataCenter:  dataCenter,
   })
   \`\`\`

6. **复杂逻辑和算法**
   \`\`\`go
   // 计算 Needle 在文件中的实际偏移量
   // SeaweedFS 使用 8 字节对齐，所以需要计算 padding
   // 公式：actualOffset = SuperBlockSize + offset + padding
   actualOffset := int64(SuperBlockSize)  // 跳过 SuperBlock（8 字节）
   actualOffset += int64(offset) * NeedleEntrySize  // Needle 索引偏移

   // 计算对齐 padding，确保 8 字节边界
   // 例如：size=13 时，padding = (8 - 13%8) % 8 = 3
   padding := (NeedlePaddingSize - actualOffset%NeedlePaddingSize) % NeedlePaddingSize
   actualOffset += padding
   \`\`\`

7. **错误处理和边界情况**
   \`\`\`go
   // 读取 Needle 数据
   n, err := volume.ReadNeedle(needleId)
   if err != nil {
       // 可能的错误情况：
       // 1. Needle 不存在（已删除或从未创建）
       // 2. 磁盘 I/O 错误
       // 3. 数据损坏（CRC 校验失败）
       if err == ErrorNotFound {
           return nil, fmt.Errorf("Needle %d 不存在", needleId)
       }
       return nil, fmt.Errorf("读取失败: %v", err)
   }
   \`\`\`

## 注释详细程度

* **简单代码**：一行注释说明目的
* **中等复杂**：多行注释解释逻辑 + 参数说明
* **复杂逻辑**：分步注释 + 原理说明 + 示例 + 边界情况

**示例（复杂函数）**：
\`\`\`go
// parseReplicaPlacement 解析副本放置策略字符串
// SeaweedFS 使用三位数字表示副本策略：XYZ
//   - X: 不同数据中心的副本数
//   - Y: 不同机架的副本数（同数据中心）
//   - Z: 不同服务器的副本数（同机架）
//
// 示例：
//   - "000": 无副本
//   - "001": 同机架不同服务器 1 个副本
//   - "010": 同数据中心不同机架 1 个副本
//   - "100": 不同数据中心 1 个副本
//   - "200": 不同数据中心 2 个副本
//
// 参数:
//   - rp: 副本策略字符串，必须是 3 位数字
// 返回:
//   - *ReplicaPlacement: 解析后的副本策略对象
//   - error: 格式错误时返回
func parseReplicaPlacement(rp string) (*ReplicaPlacement, error) {
    // 验证格式：必须是 3 位数字
    if len(rp) != 3 {
        return nil, fmt.Errorf("副本策略必须是 3 位数字，当前: %s", rp)
    }

    // 解析每一位数字
    // rp[0] - 数据中心级别副本数（X）
    dataCenterCount := int(rp[0] - '0')
    // rp[1] - 机架级别副本数（Y）
    rackCount := int(rp[1] - '0')
    // rp[2] - 服务器级别副本数（Z）
    serverCount := int(rp[2] - '0')

    // 边界检查：每个级别副本数不能超过 9
    if dataCenterCount > 9 || rackCount > 9 || serverCount > 9 {
        return nil, fmt.Errorf("副本数不能超过 9")
    }

    return &ReplicaPlacement{
        DataCenterCount: dataCenterCount,
        RackCount:       rackCount,
        ServerCount:     serverCount,
    }, nil
}
\`\`\`

## 安全原则

* **不修改**原有代码逻辑和功能
* **不添加**新的功能代码（除非用户明确要求）
* **不重构**现有代码结构
* **不破坏**编译和运行（添加的注释不能导致语法错误）
* 遵循 Go 语言注释规范（\`//\` 单行注释，\`/* */\` 多行注释）
这是Go语言的，如果是其他语言，注释的细则方式不变，但是要遵循该项目语言注释规范`;

export function createDefaultCommenterProjectProfileDraft(): CommenterProjectProfileDraft {
  return {
    project_key: 'demo-project',
    profile_name: '示例项目',
    root_path: '',
    include_extensions: ['go', 'ts', 'json'],
    exclude_directories: ['node_modules', 'dist'],
    prompt_template: DEFAULT_COMMENT_PROMPT_TEMPLATE,
    settings: {
      default_run_mode: 'review',
      default_max_workers: 2,
      default_max_retries: 1,
      default_max_files: 0,
      allow_light_rewrite: true,
      json_handling_strategy: 'sidecar_only',
      api_base_url: 'https://unvcoding.copilot.qq.com',
      api_model: 'glm-5.1',
      request_timeout_secs: 600
    }
  };
}

export const defaultCommenterProjectProfileDraft = createDefaultCommenterProjectProfileDraft();
