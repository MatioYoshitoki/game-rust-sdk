from verify_sdk_5_88_interception_browser import verify_interception_matrix
from verify_sdk_5_88_paid_browser import verify_paid_matrix
from verify_sdk_5_88_activation_browser import verify_activation_matrix
from verify_sdk_5_88_counter_browser import verify_counter_matrix
"""Ordinary-clock four/eight page consumption of generic combat and compound states."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
from uuid import uuid4
from playwright.sync_api import Error, sync_playwright
from verify_sdk_5_45_browser import journey
from verify_sdk_5_88_reservation_browser import verify_reservations
from verify_sdk_5_88_steal_browser import verify_resource_steal
from verify_sdk_5_88_power_browser import verify_character_power
from verify_sdk_5_88_derived_browser import verify_derived_matrix
from verify_sdk_5_88_gates_browser import verify_source_matrix
from verify_sdk_5_88_unit_browser import verify_unit_effects
from verify_sdk_5_88_ally_browser import verify_ally_commit,verify_lifecycle_transfer,verify_control_policy,verify_control_policy_timing,verify_hit_status,verify_contact_modifier,verify_chain_damage,verify_post_consumption,verify_contact_reaction,verify_finisher,verify_finisher_followup,verify_action_enhancement,verify_cast_lifecycle,verify_periodic_composition,verify_skill_loadouts

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT/'target/debug/kernel-host')
    parser.add_argument('--server', type=Path, default=ROOT/'adapters/h5/skill-hp-cost-server.mjs')
    parser.add_argument('--configs', type=Path, default=ROOT/'examples/combat-host')
    parser.add_argument('--out', type=Path, default=ROOT/'artifacts/sdk-5-88/browser')
    parser.add_argument('--cwd', type=Path, default=ROOT)
    parser.add_argument('--port', type=int, default=4360)
    args = parser.parse_args()
    for name in ['binary','server','configs','out','cwd']:
        setattr(args, name, getattr(args, name).resolve())
    args.out.mkdir(parents=True, exist_ok=True)
    database=args.out/f'rooms-{uuid4().hex}.sqlite'
    address=f'http://127.0.0.1:{args.port}'
    command=['node',str(args.server),'--binary',str(args.binary),'--configs',str(args.configs/'catalog.json'),
             '--host-config',str(args.configs/'host.json'),'--database',str(database),'--port',str(args.port),'--trusted-control-stdin']
    with (args.out/'server.log').open('w') as log:
        server=subprocess.Popen(command,cwd=args.cwd,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
        def trusted_control(room_id,participant_id,body):
            log_path=args.out/'server.log';offset=len(log_path.read_text().splitlines())
            server.stdin.write(json.dumps({'kind':'set_skill_loadout','room_id':room_id,'participant_id':participant_id,**body})+'\n');server.stdin.flush();deadline=time.monotonic()+10
            while True:
                for line in log_path.read_text().splitlines()[offset:]:
                    try:row=json.loads(line)
                    except ValueError:continue
                    if row.get('kind')=='trusted_control_result' and row.get('operation_id')==body['operation_id']:return row
                assert server.poll() is None and time.monotonic()<deadline
                time.sleep(.02)
        try:
            with sync_playwright() as p:
                request=p.request.new_context(base_url=address)
                deadline=time.monotonic()+20
                while True:
                    try:
                        response=request.get('/api/config',timeout=1000)
                    except Error:
                        response=None
                    if response and response.ok: break
                    assert server.poll() is None and time.monotonic()<deadline
                    time.sleep(0.1)
                assert response.json()['sdk']['combatRules']['minimumMechanismVersion']==105
                browser=p.chromium.launch()
                cases=[]
                for scenario,seats,bps,expected in [('four',4,5000,[80,153,80,102]),('eight',8,10000,[80,196,80,98])]:
                    case=journey(browser,request,address,args.out,scenario,seats,0,0,[0,0],
                                 skill_power=(bps,expected[1],expected,1,True))
                    cases.append(case)
                    print(json.dumps(case,ensure_ascii=False),flush=True)
                ally_cases=[verify_ally_commit(browser,request,address,scenario,seats) for scenario,seats in [("four",4),("eight",8)]]
                lifecycle_cases=[verify_lifecycle_transfer(browser,request,address,scenario,seats) for scenario,seats in [("four",4),("eight",8)]]
                policy_cases=[verify_control_policy(browser,request,address,scenario,seats) for scenario,seats in [("four",4),("eight",8)]]
                hit_cases=[verify_hit_status(browser,request,address,scenario,seats) for scenario,seats in [('marks-four',4),('marks-eight',8)]]
                contact_cases=[verify_contact_modifier(browser,request,address,scenario,seats) for scenario,seats in [('strikes-four',4),('strikes-eight',8)]]
                chain_cases=[verify_chain_damage(browser,request,address,scenario,seats) for scenario,seats in [('chain-four',4),('chain-eight',8)]]
                post_cases=[verify_post_consumption(browser,request,address,scenario,seats) for scenario,seats in [('consumed-four',4),('consumed-eight',8)]]
                reaction_cases=[verify_contact_reaction(browser,request,address,scenario,seats) for scenario,seats in [('reaction-four',4),('reaction-eight',8)]]
                finisher_cases=[verify_finisher(browser,request,address,scenario,seats) for scenario,seats in [('finisher-four',4),('finisher-eight',8)]]
                finisher_followup_cases=[verify_finisher_followup(browser,request,address,scenario,seats) for scenario,seats in [('followup-four',4),('followup-eight',8)]]
                action_enhancement_cases=[verify_action_enhancement(browser,request,address,scenario,seats) for scenario,seats in [('enhancement-four',4),('enhancement-eight',8)]]
                periodic_composition_cases=[verify_periodic_composition(browser,request,address,scenario,seats) for scenario,seats in [('composed-four',4),('composed-eight',8)]]
                skill_loadout_cases=[verify_skill_loadouts(browser,request,address,scenario,seats,trusted_control) for scenario,seats in [('loadouts-four',4),('loadouts-eight',8)]]
                cast_lifecycle_cases=[verify_cast_lifecycle(browser,request,address,scenario,seats) for scenario,seats in [('casting-four',4),('casting-eight',8)]]
                unit_effect_cases=[verify_unit_effects(browser,request,address,scenario,seats) for scenario,seats in [("units-four",4),("units-eight",8)]]
                reservation_cases=[verify_reservations(browser,request,address,scenario,seats) for scenario,seats in [("reservations-four",4),("reservations-eight",8)]]
                resource_steal_cases=[verify_resource_steal(browser,request,address,scenario,seats) for scenario,seats in [("steal-four",4),("steal-eight",8)]]
                character_power_cases=[verify_character_power(browser,request,address,scenario,seats) for scenario,seats in [("power-four",4),("power-eight",8)]]
                browser.close()
                request.dispose()
            report={'passed':True,'database':str(database),'binarySha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'cases':cases,'characterPowerCases':character_power_cases,'resourceStealCases':resource_steal_cases,'reservationCases':reservation_cases,'unitEffectCases':unit_effect_cases,'skillLoadoutCases':skill_loadout_cases,'periodicCompositionCases':periodic_composition_cases,'castLifecycleCases':cast_lifecycle_cases,'allySupportCases':ally_cases,'lifecycleCases':lifecycle_cases,'controlPolicyCases':policy_cases,'hitStatusCases':hit_cases,'contactModifierCases':contact_cases,'chainDamageCases':chain_cases,'postConsumptionCases':post_cases,'contactReactionCases':reaction_cases,'finisherCases':finisher_cases,'finisherFollowupCases':finisher_followup_cases,'actionEnhancementCases':action_enhancement_cases}
        finally:
            server.terminate()
            server.wait(timeout=10)
    report["derivedEffectsCases"]=verify_derived_matrix(args)
    report["sourceGateCases"]=verify_source_matrix(args)
    report["stackWindowCases"]=verify_counter_matrix(args)
    report["conditionalActivationCases"]=verify_activation_matrix(args)
    report["paidResourceCases"]=verify_paid_matrix(args)
    report["castInterceptionCases"]=verify_interception_matrix(args)
    report["rooms"]=sum(len(v) for k,v in report.items() if k=="cases" or k.endswith("Cases"))
    verify_control_policy_timing(policy_cases,database)
    (args.out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')

if __name__=='__main__': main()
