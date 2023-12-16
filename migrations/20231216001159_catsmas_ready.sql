-- Add migration script here
ALTER TABLE `catsmas_users`
ADD ready boolean NOT NULL DEFAULT false;