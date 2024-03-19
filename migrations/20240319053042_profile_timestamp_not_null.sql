-- Add migration script here
ALTER TABLE `profiles`
MODIFY `created_at` DATETIME NOT NULL;
ALTER TABLE `profiles`
MODIFY `updated_at` DATETIME NOT NULL;