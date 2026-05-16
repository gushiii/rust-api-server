CREATE DATABASE test_db;
USE test_db;

CREATE TABLE `products` (
  `product_uuid` VARCHAR(64) PRIMARY KEY,
  `title` VARCHAR(100) NOT NULL,
  `price` INT DEFAULT 0
);

INSERT INTO `products` (`product_uuid`, `title`, `price`) VALUES
('pk_001', 'iPhone 15 Pro Max 256GB', 9999),
('pk_002', 'Sony WH-1000XM5 无线降噪耳机', 2299),
('pk_003', 'Nike Air Max 90 运动鞋', 899),
('pk_004', 'Logitech MX Master 3S 无线鼠标', 699),
('pk_005', 'Kindle Paperwhite 5 电子书阅读器', 1068),
('pk_006', 'Nintendo Switch OLED 游戏主机', 2299),
('pk_007', '星巴克经典陶瓷马克杯 400ml', 129),
('pk_008', '戴森 Dyson V12 吸尘器', 3999),
('pk_009', '优衣库 UNIQLO 男装纯棉短袖 T 恤', 99),
('pk_010', '安克 Anker 20000mAh 移动电源', 199),
('pk_011', 'Le Labo Santal 33 檀香木香水 50ml', 1650),
('pk_012', '斐尔可 FILCO 圣手二代机械键盘', 1099),
('pk_013', 'Stanley 保温保冷吸管杯 1.2L', 348),
('pk_014', 'Bose SoundLink Flex 便携蓝牙音箱', 1099),
('pk_015', '无印良品 MUJI 超声波香薰机', 380),
('pk_016', 'Nespresso 雀巢胶囊咖啡机 Vertuo', 1288),
('pk_017', 'Patagonia Torrentshell 3L 防水外套', 1399),
('pk_018', 'iPad Air 11 英寸 M2 芯片 128GB', 4799),
('pk_019', 'Steam Deck OLED 掌上游戏机 512GB', 4299),
('pk_020', '乐高 LEGO 机械组保时捷 911 积木', 1399);
