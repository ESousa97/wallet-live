// Dois tipos de autenticação: a do admin (secret key no header) e a do usuário
// final do sistema (nome + senha hasheada no banco).
pub mod admin;
pub mod user;
