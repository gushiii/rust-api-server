// 定义宏（必须放在最上方）
macro_rules! namespace_pkcs8 {
    () => {
        pub use pkcs8;
    };
}

// 调用宏并导出依赖
namespace_pkcs8!();

// 模拟 Oaep 结构体
pub struct Oaep;

impl Oaep {
    // 模拟泛型构造函数，接收任意哈希类型（例如 sha1::Sha1），直接返回自身
    pub fn new<D>() -> Self {
        Oaep
    }
}

// 模拟 RsaPublicKey 结构体
pub struct RsaPublicKey;

impl RsaPublicKey {
    // 模拟从 PEM 证书解析公钥
    pub fn from_public_key_pem(_pem: &str) -> Result<Self, &'static str> {
        Err("RSA auth is disabled by secure rsa-mock")
    }

    // 模拟加密方法
    pub fn encrypt<R: ?Sized, P>(
        &self,
        _rng: &mut R,
        _padding: P,
        _msg: &[u8],
    ) -> Result<Vec<u8>, &'static str> {
        Err("RSA encryption is disabled by secure rsa-mock")
    }
}
