"""React to a specific native gameplay event using ordinary browser controls."""


import json

def observe_resource_actions(page, key):
    page.add_init_script("""(key=>{
      const actionKey=key+'-reaction';
      window.__probeEvents=JSON.parse(sessionStorage.getItem(key)||'[]');
      window.__armResourceReaction=control=>{
        if(window.__resourceReaction) throw Error('resource reaction already armed');
        window.__resourceReaction=control;
        sessionStorage.removeItem(actionKey);
      };
      window.__readResourceReaction=()=>JSON.parse(sessionStorage.getItem(actionKey)||'null');
      const Native=window.EventSource;
      window.EventSource=class extends Native {
        constructor(...args){
          super(...args);
          this.addEventListener('gameplay',message=>{
            const row=JSON.parse(message.data);
            window.__probeEvents.push(row);
            sessionStorage.setItem(key,JSON.stringify(window.__probeEvents));
            const event=row.kind==='private'?row.event:null;
            if(event?.kind!=='resource_stolen'||!window.__resourceReaction)return;
            const control=window.__resourceReaction;
            window.__resourceReaction=null;
            const snapshot=window.__lastSnapshot;
            sessionStorage.setItem(actionKey,JSON.stringify({
              actionId:event.action_id,control,
              snapshotLogicalMs:snapshot?.logical_ms,
              documentVisibility:document.visibilityState,
              disabled:control!=='reload'&&document.getElementById(control).disabled,
              readyAt:snapshot?.self.player_skills[0]?.ready_at_ms
            }));
            if(control==='reload')location.reload();
            else document.getElementById(control).click();
          });
        }
      };
    })(%s);""" % json.dumps(key))


def arm_resource_action(page, control):
    assert control in ('cancel','leave','reload')
    page.evaluate('control=>window.__armResourceReaction(control)', control)


import json

def observed_resource_action(page, control, action_id):
    page.wait_for_function(
        'q=>{const r=window.__readResourceReaction?.();return r?.control===q[0]&&r.actionId===q[1]}',
        arg=[control,action_id])
    return page.evaluate('window.__readResourceReaction()')
