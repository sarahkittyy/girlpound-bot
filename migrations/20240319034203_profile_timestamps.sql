-- Add migration script here
ALTER TABLE `profiles`
ADD COLUMN `created_at` DATETIME DEFAULT CURRENT_TIMESTAMP,
ADD COLUMN `updated_at` DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP;