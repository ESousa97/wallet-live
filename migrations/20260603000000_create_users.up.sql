CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY NOT NULL,
    -- username é a chave de login do usuário (como um e-mail seria): tem que ser
    -- único entre todos os usuários.
    username TEXT NOT NULL UNIQUE,
    -- Nunca guardamos a senha em texto livre: armazenamos só a hash gerada pela
    -- biblioteca de autenticação.
    password_hash TEXT NOT NULL
);
