CREATE DATABASE test_db;
USE test_db;

CREATE TABLE `products` (
  `product_uuid` VARCHAR(64) PRIMARY KEY,
  `title` VARCHAR(100) NOT NULL,
  `price` INT DEFAULT 0
);