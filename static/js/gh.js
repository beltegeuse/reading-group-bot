// Bulma navbar burger menu toggle
document.addEventListener('DOMContentLoaded', () => {
  const $navbarBurgers = Array.prototype.slice.call(document.querySelectorAll('.navbar-burger'), 0);

  $navbarBurgers.forEach( el => {
    el.addEventListener('click', () => {
      const target = el.dataset.target;
      const $target = document.getElementById(target);
      el.classList.toggle('is-active');
      $target.classList.toggle('is-active');
    });
  });
});

// Close notification when delete button is clicked
document.addEventListener('DOMContentLoaded', () => {
  (document.querySelectorAll('.notification .delete') || []).forEach(($delete) => {
    const $notification = $delete.parentNode;

    $delete.addEventListener('click', () => {
      $notification.parentNode.removeChild($notification);
    });
  });
});

// Update file input display name
document.addEventListener('DOMContentLoaded', () => {
  const fileInputs = document.querySelectorAll('.file-input');
  
  fileInputs.forEach(fileInput => {
    fileInput.addEventListener('change', (event) => {
      const fileName = event.target.files[0]?.name || 'No file selected';
      const fileNameElement = fileInput.closest('.file-label').querySelector('.file-name');
      if (fileNameElement) {
        fileNameElement.textContent = fileName;
      }
    });
  });
});

// Dynamic filter for "only not voted" on index page
document.addEventListener('DOMContentLoaded', () => {
  const notVotedToggle = document.getElementById('only-not-voted-toggle');
  const paperCards = Array.from(document.querySelectorAll('.paper-card'));
  const dynamicNoResults = document.getElementById('dynamic-no-results');

  if (!notVotedToggle || paperCards.length === 0) {
    return;
  }

  const applyNotVotedFilter = () => {
    const onlyNotVoted = notVotedToggle.checked;

    paperCards.forEach((card) => {
      const voteState = Number(card.dataset.voteState || '0');
      const visible = !onlyNotVoted || voteState === 0;
      card.style.display = visible ? '' : 'none';
    });

    if (dynamicNoResults) {
      const visibleCardsCount = paperCards.filter((card) => card.style.display !== 'none').length;
      dynamicNoResults.classList.toggle('is-hidden', visibleCardsCount > 0);
    }
  };

  notVotedToggle.addEventListener('change', applyNotVotedFilter);
  applyNotVotedFilter();
});
