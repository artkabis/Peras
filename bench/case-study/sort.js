// The comparison sort Peras does not have yet, from JavaScript.
export const sortByAmount = (orders, up) =>
  orders.slice().sort((a, b) => (up ? a.amount - b.amount : b.amount - a.amount));
