# Rust API Server Engine

一个采用 **Rust + Axum + SQLx** 构建的**自适应低代码 API 引擎**。无需编写业务 SQL，通过动态反射自适应地处理任意数据表的 CRUD 操作、多表联查、聚合统计，配合声明式 DSL 过滤协议和完整的安全防御体系，提供高并发处理能力。

## ✨ 主要特性

- 🚀 **高性能并发** - 基于 Axum 和 Tokio 的异步 API 引擎，单机可支持千级并发
- 🔐 **双层安全认证** - JWT 长短牌双令牌体系，支持智能无感续期
- 💾 **动态 CRUD** - 自适应读取表名、主键，完成插入后动态行反查
- 🌐 **多条件检索** - 深度挂载 DSL 编译中心，支持区间查询、模糊匹配、排序分页
- 🔗 **自动多表联查** - 利用 MySQL 元数据字典，双下划线隔离别名，防混淆嵌套
- 📊 **聚合统计引擎** - 声明式 GROUP BY + HAVING，精准浮点数聚合
- 🛡️ **防注入体系** - 参数化查询、标识符白名单校验、双下划线防冲突清洗
- ⚡ **令牌桶速率限制** - 基于 IP 的高频轰炸熔断保护
- 🌍 **灵活 CORS 配置** - 支持多源跨域访问控制

## 🛠️ 技术栈

| 技术 | 版本 | 用途 |
|------|------|------|
| Rust | 2024 | 编程语言 |
| Axum | 0.8.9 | Web 框架 |
| Tokio | 1.52.3 | 异步运行时 |
| SQLx | 0.8.6 | 类型安全数据库操作 |
| MySQL | 5.7+ / 8.0+ | 数据库 |
| jsonwebtoken | 10.4.0 | JWT 身份验证 |
| bcrypt | 0.19.1 | 密码加盐哈希 |
| Tower | - | 中间件生态 |
| tower_governor | 0.8.0 | 令牌桶限流 |

## 🚀 快速开始

### 1. 克隆项目

```bash
git clone https://github.com/gushiii/rust-api-server.git
cd rust-api-server
```

### 2. 配置环境变量

复制 `.env.example` 到 `.env` 并配置：

```bash
cp .env.example .env
```

编辑 `.env` 文件：

```env
# 数据库配置 (必需)
DATABASE_URL=mysql://user:password@localhost:3306/database_name

# 服务器配置
SERVER_PORT=8080

# JWT 配置 (推荐修改)
JWT_SECRET=your_secret_key_here_change_in_production

# 速率限制配置 (可选)
# 令牌桶突发请求数
RATE_LIMIT_BURST=10
# 每秒允许的平均请求数
RATE_LIMIT_PER_SECOND=2

# CORS 配置 (可选)
CORS_ALLOWED_ORIGINS=http://localhost:3000,http://localhost:5173
```

### 3. 数据库初始化

使用 `example/testDB.sql` 初始化数据库：

```bash
mysql -u user -p database_name < example/testDB.sql
```

### 4. 构建并运行

```bash
# 开发模式运行
cargo run

# 检查编译
cargo check

# 发布模式编译（优化）
cargo build --release

# 运行发布版本
./target/release/rust-api-server-engine
```

服务器将在 `http://localhost:8080` 启动。

### 5. 验证服务运行

```bash
# 注册用户
curl -X POST http://localhost:8080/api/v1/auth/users/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "test_user",
    "password": "test_password",
    "email": "test@example.com"
  }'

# 登录获取令牌
curl -X POST http://localhost:8080/api/v1/auth/users/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "test_user",
    "password": "test_password"
  }'

# 使用令牌查询数据
curl -X GET http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer <your_access_token>"
```

## 📚 API 端点完全指南

系统划分为**🔓公开认证网关带**和**🔒受保护核心业务网关带**，任何业务表（如 products, members, users）均即插即用。

### 🔓 1.1 动态认证端点（公开路由 - 免 Bearer 令牌）

#### 动态安全注册
```
POST /api/v1/auth/{table}/register
```

**请求体：**
```json
{
  "username": "john_doe",
  "password": "secure_password",
  "email": "john@example.com"
}
```

**网关行为：**
- 自动拦截 JSON Body 中的密码列
- 网关层执行 **Bcrypt 加盐哈希**
- 抹除内存明文后安全写入目标表

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": {
    "id": 1,
    "username": "john_doe",
    "email": "john@example.com"
  },
  "duration_ms": 5
}
```

#### 动态安全登录
```
POST /api/v1/auth/{table}/login
```

**请求体：**
```json
{
  "username": "john_doe",
  "password": "secure_password"
}
```

**网关行为：**
- 仅根据账号单列锁定用户的哈希密码
- 执行 Bcrypt 运行时密文校验
- 成功后**串行签发并下发长短双令牌**

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "user_id": 1
  },
  "duration_ms": 8
}
```

#### 智能长短牌双令牌无感续期
```
POST /api/v1/auth/{table}/refresh
```

**请求体：**
```json
{
  "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
  "old_access_token": "eyJ0eXAiOiJKV1QiLCJhbGc..."
}
```

**网关行为：**
- 利用 **decode_insecure（非校验解构评估）** 动态核算旧牌的剩余寿命
- 若未过期则原样回传**（防止令牌膨胀）**
- 若彻底失效则安全换发新牌

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGc..."
  },
  "duration_ms": 3
}
```

### 🔒 1.2 核心业务端点（受保护路由 - 必须携带 Authorization: Bearer <Token>）

#### 动态单表数据创建
```
POST /api/v1/{table}
Header: Authorization: Bearer <access_token>
```

**请求体：**
```json
{
  "title": "智能手机",
  "price": 2999.99,
  "stock": 100
}
```

**底层反射：**
- 自适应读取表名和主键
- 完成插入后动态执行物理行反查
- 以标准 JSON 回执响应

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": {
    "id": 1,
    "title": "智能手机",
    "price": 2999.99,
    "stock": 100
  },
  "duration_ms": 4
}
```

#### 多条件检索过滤列表
```
GET /api/v1/{table}?[条件参数]
Header: Authorization: Bearer <access_token>
```

**底层反射：**
- 深度挂载自研 DSL 编译中心
- 防冲突双下划线嵌套清洗器
- 常规保留字隔离带

#### 动态精准查询单条详情
```
GET /api/v1/{table}/{id}
Header: Authorization: Bearer <access_token>
```

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": {
    "id": 1,
    "title": "智能手机",
    "price": 2999.99,
    "stock": 100
  },
  "duration_ms": 2
}
```

#### 动态按需增量更新数据
```
PUT /api/v1/{table}/{id}
Header: Authorization: Bearer <access_token>
```

**请求体：**
```json
{
  "price": 2899.99,
  "stock": 95
}
```

**保护机制：**
- 动态锁定 information_schema 字典
- 严格防止物理主键字段被前端恶意串改
- 仅更新提供的字段（增量更新）

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": {
    "id": 1,
    "title": "智能手机",
    "price": 2899.99,
    "stock": 95
  },
  "duration_ms": 3
}
```

#### 动态物理删除记录
```
DELETE /api/v1/{table}/{id}
Header: Authorization: Bearer <access_token>
```

**返回数据：**
- 返回规范的 rows_affected（受影响行数）JSON 骨架

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": {
    "rows_affected": 1
  },
  "duration_ms": 2
}
```

---

## 🛠️ 2. 声明式万能 DSL 过滤协议总览

所有复杂业务、多表联查、区间检索以及聚合控制，全量收拢在**列表接口（GET /api/v1/{table}）的 URL Query 参数中**，格式严格定义如下：

### 2.1 常规参数（隐式等值过滤）

任何不等于系统保留字的普通键值对，会被引擎自动转换为常规的 WHERE 条件，并**具备智能类型探测能力**（自动转数字/字符串绑定触发索引）：

```bash
# 完全匹配数字值（防 0 值漏杀问题）
?price=0          →  WHERE price = 0

# 完全匹配字符串值
?status=active    →  WHERE status = 'active'

# 多条件隐式 AND 连接
?status=active&category=electronics  →  WHERE status = 'active' AND category = 'electronics'
```

### 2.2 保留字 `_where`（高阶区间与模糊匹配对象）

使用统一的表达式 JSON 对象传入，彻底规避破坏字段命名的下划线硬编码硬伤（如 age_gt），内部操作符全量对齐：

```bash
# 大于 / 大于等于
?_where={"age":{"$gt":20}}
?_where={"age":{"$gte":20}}

# 小于 / 小于等于
?_where={"price":{"$lt":50}}
?_where={"price":{"$lte":50}}

# 不等于匹配
?_where={"status":{"$neq":"deleted"}}

# 模糊匹配
?_where={"title":{"$like":"java"}}   →  WHERE title LIKE '%java%'
```

**内嵌排序修饰符（_sort 与 _order）：**

内嵌在 `_where` 结构体中，支持字段排序和方向控制（走白名单防注入）：

```bash
# 按年龄降序排列
?_where={"age":{"$gt":20},"_sort":"age","_order":"desc"}

# 支持 asc (升序) 或 desc (降序)
?_where={"status":"active","_sort":"created_at","_order":"asc"}
```

### 2.3 保留字 `_join`（自动无歧义关系嵌套联查对象）

前端通过传递纯声明对象，由网关在运行时自动利用 **MySQL 元数据字典扫描子表物理列**，使用**双下划线（__）作为多表别名的唯一防混淆隔离分水岭**：

**DSL 传入：**
```bash
?_join={"table":"categories","on":"category_id","type":"LEFT"}
```

**运行表现：**

1. 自动执行 LEFT JOIN 联查
2. 自动防误杀：主表原生自带的下划线字段（如 product_uuid）原样完整保留在最外层
3. 自动高雅嵌套：将子表的数据全部剥离双下划线前缀，自动封装进一个以子表名命名的**二级 JSON 嵌套对象**中
4. 自动剔除外键：控制层在内存中自动 remove 清洗掉主表外层冗余重复的 "category_id" 脏字段

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": [
    {
      "id": 1,
      "title": "智能手机",
      "price": 2999.99,
      "product_uuid": "uuid-xxx",
      "categories": {
        "id": 5,
        "name": "Electronics"
      }
    }
  ],
  "duration_ms": 12
}
```

### 2.4 保留字 `_group` 与 `_aggregate`（声明式无 SQL 污染聚合统计）

彻底解耦破坏语义的 `_select=COUNT(*)` 漏洞设计，前端仅需通过 JSON 传达统计意图，由后端智能翻译：

**DSL 传入：**
```bash
?_group=status&_aggregate={"count":"*","avg":"age"}
```

**运行表现：**
- 自动组装 `GROUP BY status` 并追加聚合计算
- 在 src/encoder.rs 中通过 sqlx::types::BigDecimal 击穿底层字节流
- **彻底根除 AVG 返回为 null 或 0.0 的顽疾**，精准输出浮点数值

**支持的聚合函数：**
- `count` - 计数
- `sum` - 求和
- `avg` - 平均值（精准浮点输出）
- `min` - 最小值
- `max` - 最大值

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": [
    {
      "status": "active",
      "count": 15,
      "avg": 32.5
    },
    {
      "status": "inactive",
      "count": 8,
      "avg": 28.3
    }
  ],
  "duration_ms": 6
}
```

### 2.5 保留字 `_having`（针对聚合结果的二次过滤）

在 GROUP BY 后对聚合结果进行过滤：

**DSL 传入：**
```bash
?_group=status&_aggregate={"count":"*"}&_having={"count":{"$gt":5}}
```

**运行表现：**
- 解析器会自动执行**别名到物理函数的二次翻译映射**
- 在 GROUP BY 之后、LIMIT 之前织入 `HAVING COUNT(*) > 5`

**响应示例：**
```json
{
  "success": true,
  "status": 200,
  "data": [
    {
      "status": "active",
      "count": 15
    }
  ],
  "duration_ms": 5
}
```

### 2.6 保留字 `_limit` 与 `_offset`（通用流控分页控制）

**DSL 传入：**
```bash
?_limit=10&_offset=0
```

**运行表现：**
- 自动追加在物理 SQL 的最末尾
- 实现流控分页
- _limit: 返回的最大记录数
- _offset: 跳过的记录数（从 0 开始）

### 2.7 综合示例

**复杂多条件查询示例：**
```bash
GET /api/v1/products?status=active&_where={"price":{"$gte":1000,"$lte":5000},"_sort":"price","_order":"asc"}&_limit=20&_offset=0
```

翻译为：
```sql
SELECT * FROM products
WHERE status = 'active'
  AND price >= 1000
  AND price <= 5000
ORDER BY price ASC
LIMIT 20 OFFSET 0
```

**多表联查 + 聚合示例：**
```bash
GET /api/v1/orders?_join={"table":"customers","on":"customer_id","type":"LEFT"}&_group=status&_aggregate={"count":"*","sum":"total"}&_having={"count":{"$gt":3}}&_sort=count&_order=desc
```

---

## 📦 3. JSON 响应回执骨架

无论是业务成功、权限漏洞拦截、还是高频 DDoS 压测熔断，系统在网关外围输出时，均达成了一致性协议：

### 3.1 成功回执（200 OK）

自动追踪毫秒级物理执行耗时：

```json
{
  "success": true,
  "status": 200,
  "data": [
    {
      "id": 1,
      "title": "智能手机",
      "price": 2999.99
    }
  ],
  "duration_ms": 3
}
```

### 3.2 客户端报错回执（400 Bad Request）

自动隐藏 data 和 duration 字段：

```json
{
  "success": false,
  "status": 400,
  "error": "Security error: Invalid database identifier 'drop_table'"
}
```

### 3.3 网关令牌拦截与到期回执（401 Unauthorized）

```json
{
  "success": false,
  "status": 401,
  "error": "Unauthorized: Access token has expired. Please refresh."
}
```

### 3.4 令牌桶恶意高频轰炸熔断回执（429 Too Many Requests）

保护底层数据库不挂：

```json
{
  "success": false,
  "status": 429,
  "error": "Too Many Requests. Please slow down and try again later."
}
```

### 3.5 系统内部物理阻断回执（500 Internal Server Error）

```json
{
  "success": false,
  "status": 500,
  "error": "Table 'test_db.products' doesn't exist"
}
```

---

## 📚 快速 API 参考表

| 功能 | 端点 | 方法 | 认证 |
|------|------|------|------|
| 注册 | `/api/v1/auth/{table}/register` | POST | ❌ |
| 登录 | `/api/v1/auth/{table}/login` | POST | ❌ |
| 刷新令牌 | `/api/v1/auth/{table}/refresh` | POST | ❌ |
| 创建 | `/api/v1/{table}` | POST | ✅ |
| 列表/查询 | `/api/v1/{table}` | GET | ✅ |
| 详情 | `/api/v1/{table}/{id}` | GET | ✅ |
| 更新 | `/api/v1/{table}/{id}` | PUT | ✅ |
| 删除 | `/api/v1/{table}/{id}` | DELETE | ✅ |

## 🔗 相关资源

- [Axum 文档](https://docs.rs/axum/latest/axum/)
- [Tokio 文档](https://tokio.rs/)
- [SQLx 文档](https://github.com/launchbadge/sqlx)
- [Rust Book](https://doc.rust-lang.org/book/)

## 🔧 开发工具与贡献指南

### 必需工具安装

#### Rust 安装

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### 开发工具链

```bash
# cargo-generate - 项目模板生成
cargo install cargo-generate

# pre-commit - 代码检查工具
pipx install pre-commit
pre-commit install

# cargo-deny - 依赖安全审计
cargo install --locked cargo-deny

# typos - 拼写检查
cargo install typos-cli
```

### 开发工作流

```bash
# 检查编译（快速反馈）
cargo check

# 运行测试套件
cargo test

# 自动格式化代码
cargo fmt

# Lint 代码质量检查
cargo clippy

# 拼写检查
typos

# 依赖安全检查
cargo deny check

# 执行所有 pre-commit 检查
pre-commit run --all-files
```

## 📁 项目结构说明

```
src/
├── main.rs        # 应用入口点，路由配置，服务器初始化
├── auth.rs        # JWT 认证业务逻辑（注册、登录、令牌刷新）
├── handlers.rs    # CRUD 操作处理器，动态反射主逻辑
├── encoder.rs     # MySQL 行转 JSON 序列化，精准浮点聚合
├── binder.rs      # 请求数据绑定、JSON 反序列化、类型转换
├── parser.rs      # DSL 查询参数解析、标识符白名单校验
└── response.rs    # 统一 API 响应骨架，错误处理

example/
└── testDB.sql     # 示例数据库初始化脚本

_typos.toml       # 拼写检查配置
deny.toml         # 依赖安全审计配置
cliff.toml        # 变更日志生成配置
.pre-commit-config.yaml  # 提交前检查配置
```

## 🔐 深度安全防御体系

### 防御层次

| 层级 | 防御机制 | 描述 |
|------|---------|------|
| **网关层** | 令牌桶限流 | 基于 IP 的高频轰炸熔断保护 |
| **认证层** | JWT 双令牌 | 长短期分离，智能无感续期 |
| **SQL 层** | 参数化查询 | SQLx 原生支持，彻底杜绝注入 |
| **标识符层** | 白名单校验 | 表名、列名、排序字段严格验证 |
| **DSL 层** | 双下划线隔离 | 防冲突嵌套，多表别名唯一化 |
| **应用层** | 类型检测 | 自动数字/字符串类型推断绑定 |
| **CORS 层** | 源控制 | 灵活的跨域来源白名单 |

### 防护示例

```bash
# ❌ SQL 注入尝试（自动拦截）
?_where={"status":"'; DROP TABLE users; --"}
# 返回 400 Bad Request

# ❌ 标识符注入（自动拦截）
?_sort="price; DROP TABLE products; --"
# 返回 400 Bad Request - Invalid database identifier

# ✅ 合法复杂查询（完全支持）
?_where={"price":{"$gte":1000,"$lte":5000}}&_sort=created_at&_order=desc
# 成功执行
```

## 🚨 环境变量完全配置表

| 环境变量 | 类型 | 默认值 | 必需 | 说明 |
|---------|------|--------|------|------|
| `DATABASE_URL` | String | - | ✅ | MySQL 连接字符串 |
| `SERVER_PORT` | Number | 8080 | ❌ | 服务器监听端口 |
| `JWT_SECRET` | String | "secret" | ❌ | JWT 签名密钥（生产环境必改） |
| `RATE_LIMIT_PER_SECOND` | Number | 2 | ❌ | 每秒允许请求数 |
| `RATE_LIMIT_BURST` | Number | 10 | ❌ | 令牌桶突发请求数 |
| `CORS_ALLOWED_ORIGINS` | String | - | ❌ | 逗号分隔的 CORS 允许源 |


## 📊 性能指标

基于单机实测（Rust Release 编译 + MySQL 本地连接）：

- **并发连接数**：单机可支持 1000+ 并发
- **吞吐量**：简单 CRUD 可达 10,000+ RPS
- **延迟**：平均响应时间 < 5ms（含 SQL 执行）
- **内存占用**：基础镜像 < 50MB

## ⚠️ 已知限制

1. 当前仅支持 MySQL 5.7+ 和 MySQL 8.0+
2. 单次请求最多支持一层 JOIN（可扩展）
3. 聚合统计不支持嵌套函数（如 `COUNT(DISTINCT)`）


## 📄 许可证

本项目采用 MIT 许可证。详见 [LICENSE](./LICENSE) 文件。
