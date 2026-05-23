mod rubyfmt;

use crate::{Error, GlobalArgs};
use rubyfmt::CommandlineOpts;

pub(crate) type FmtArgs = CommandlineOpts;

pub(crate) async fn fmt(_global_args: &GlobalArgs, opts: FmtArgs) -> Result<(), Error> {
    rubyfmt::main(opts);
    Ok(())
}
