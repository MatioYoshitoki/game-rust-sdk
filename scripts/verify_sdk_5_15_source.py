"""Test the unpacked source crate with only its delivered vendored dependencies."""
import json
import re
import tempfile
from pathlib import Path

from package_sdk import ROOT, digest, extract_crate, run


def tests(value):return sorted(re.findall(r'^test (\S+) \.\.\. ok$',value,re.M))


def main():
    output=ROOT/'artifacts/sdk-5-15/delivery'
    package=json.loads((output/'package.json').read_text())
    expected=tests((ROOT/'artifacts/sdk-5-15/default.log').read_text())
    assert len(expected)>800
    with tempfile.TemporaryDirectory(prefix='sdk-5-15-unpacked-tests-') as d:
        d=Path(d);bundle=d/'bundle';extract_crate(package['archive'],bundle,package['rootName'])
        home=d/'empty-home';home.mkdir()
        env={'CARGO_HOME':str(home),'CARGO_NET_OFFLINE':'true','CARGO_TARGET_DIR':str(d/'target')}
        results={}
        for name,flags in [('default',[]),('all-features',['--all-features'])]:
            value=run('source-'+name,['cargo','test','--locked','--offline','--manifest-path','sdk/Cargo.toml',*flags],
                      cwd=bundle,env=env,logs=output)
            assert tests(value)==expected
            results[name]=digest(output/('source-'+name+'.log'))
        run('source-clippy',['cargo','clippy','--locked','--offline','--manifest-path','sdk/Cargo.toml',
                            '--all-targets','--all-features','--','-D','warnings'],cwd=bundle,env=env,logs=output)
        results['clippy']=digest(output/'source-clippy.log')
        assert digest(Path(package['archive']))==package['sha256']
        (output/'source-verification.json').write_text(json.dumps({'passed':True,
            'archiveSha256':package['sha256'],'emptyCargoHome':True,'offline':True,'testCount':len(expected),
            'sameWorkspaceDefaultAndAllFeatureSet':True,'checks':results},indent=2)+'\n')
    print(f'Unpacked SDK-5-15: identical {len(expected)} tests and Clippy passed offline with empty Cargo home')


if __name__=='__main__':main()
