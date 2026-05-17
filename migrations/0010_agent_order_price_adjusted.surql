-- pharma-server 0010: persist price_adjusted on agent_order (Fase 11 step 4+).
-- Append-only. po.create re-quotes every line against the supplier catalog;
-- `price_adjusted` records whether any line was rewritten (price diverged,
-- product unknown or inactive). It was previously only echoed in the po.ack
-- and lost — now it is durable so a buyer polling `po.status` learns the
-- supplier repriced the order before it is accepted/fulfilled.

DEFINE FIELD price_adjusted ON agent_order TYPE bool DEFAULT false;
