// Dois tipos de autenticação: a do admin (secret key no header) e a do usuário
// final do sistema (nome + senha hasheada no banco). O módulo csrf protege os
// formulários do front-end contra requisições forjadas de outros sites.
pub mod admin;
pub mod csrf;
pub mod session;
pub mod throttle;
pub mod user;
