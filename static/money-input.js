// Máscara de valor monetário: dígito digitado acumula como centavo e o campo
// se formata sozinho (milhar com ponto, centavos com vírgula) — como um
// terminal de pagamento, não como um campo de texto livre. Só roda com
// JavaScript disponível: sem ele, o campo original (`type="number"`)
// continua exatamente como hoje, sem máscara — é o degrau de baixo, e o único
// que a suíte de front-end do servidor cobre.
//
// A validação de VALOR (positivo, escala) continua inteiramente no servidor
// (`AmountForm` em `frontend.rs`); este script só cuida de como o número
// aparece na tela e de bloquear tecla que não é dígito.
(function () {
  function attach(source) {
    if (source.dataset.moneyAttached) return;
    source.dataset.moneyAttached = "true";

    // O campo visível perde o `name` (não é mais ele que viaja no POST) e um
    // campo oculto novo herda o nome: é ele quem carrega sempre o valor
    // REAL, sem separador de milhar, no formato que o servidor já espera —
    // nenhuma mudança no lado do Rust foi necessária.
    var name = source.getAttribute("name");
    var hidden = document.createElement("input");
    hidden.type = "hidden";
    hidden.name = name;
    source.removeAttribute("name");
    source.parentNode.insertBefore(hidden, source);

    source.type = "text";
    source.setAttribute("inputmode", "numeric");
    source.setAttribute("autocomplete", "off");
    var hadFocus = document.activeElement === source;
    source.value = "";
    hidden.value = "";
    if (hadFocus) source.focus();

    // Dígitos acumulados, lidos da direita para a esquerda como centavos —
    // por isso backspace sempre remove exatamente um dígito, não um símbolo
    // de formatação.
    var raw = "";

    function render() {
      if (!raw) {
        source.value = "";
        hidden.value = "";
        return;
      }
      var reais = (parseInt(raw, 10) / 100).toFixed(2);
      var parts = reais.split(".");
      var grouped = parts[0].replace(/\B(?=(\d{3})+(?!\d))/g, ".");
      source.value = grouped + "," + parts[1];
      hidden.value = reais;
    }

    function addDigits(text) {
      var digits = text.replace(/\D/g, "");
      if (!digits) return;
      // Mesmo teto de dígitos de um valor monetário plausível — não é
      // validação de negócio (essa é do servidor), só evita um número
      // absurdo de dígitos digitados por engano ou colados.
      raw = (raw + digits).slice(0, 15);
      render();
    }

    source.addEventListener("keydown", function (event) {
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.key >= "0" && event.key <= "9") {
        event.preventDefault();
        addDigits(event.key);
      } else if (event.key === "Backspace" || event.key === "Delete") {
        event.preventDefault();
        raw = raw.slice(0, -1);
        render();
      } else if (event.key.length === 1) {
        // Um caractere que não é dígito (letra, sinal, símbolo): bloqueado.
        // Teclas de controle (Tab, setas, F5...) têm `key.length > 1` e
        // passam direto, sem cair aqui.
        event.preventDefault();
      }
    });

    source.addEventListener("paste", function (event) {
      event.preventDefault();
      var text = (event.clipboardData || window.clipboardData).getData("text");
      addDigits(text);
    });

    // Teclado virtual (celular, IME) às vezes só dispara `input`, sem um
    // `keydown` utilizável. Reforço: se o texto do campo divergir do que o
    // `raw` já sabe, resincroniza a partir dele.
    source.addEventListener("input", function () {
      if (source.value.replace(/\D/g, "") !== raw) {
        addDigits(source.value);
      }
    });
  }

  function scan(root) {
    if (root.querySelectorAll) {
      root.querySelectorAll("[data-money-input]").forEach(attach);
    }
    if (root.matches && root.matches("[data-money-input]")) {
      attach(root);
    }
  }

  // `htmx.onLoad` cobre os dois casos que importam aqui: a carga inicial da
  // página E cada fragmento novo que o htmx troca depois (reabrir o
  // formulário de depósito troca o `<main id="wallet">` inteiro, e o campo
  // de dentro precisa da máscara de novo — o listener anterior morreu junto
  // com o nó antigo).
  if (window.htmx && typeof window.htmx.onLoad === "function") {
    window.htmx.onLoad(scan);
  } else {
    document.addEventListener("DOMContentLoaded", function () {
      scan(document.body);
    });
  }
})();
