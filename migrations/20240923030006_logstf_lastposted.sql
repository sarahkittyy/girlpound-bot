-- Add migration script here
CREATE TABLE IF NOT EXISTS `logstf_lastposted` (
	`id` BIGINT NOT NULL DEFAULT 0
);
INSERT INTO `logstf_lastposted` (`id`) VALUES (3720359);