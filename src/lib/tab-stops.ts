export function getTabStops(scope: ParentNode) {
  return [...scope.querySelectorAll<HTMLElement>("button,input,textarea,a[href],summary,[tabindex]")]
    .filter(element => element.tabIndex >= 0 && !element.matches(":disabled")
      && !element.closest("[inert]") && element.checkVisibility({ visibilityProperty: true }));
}
