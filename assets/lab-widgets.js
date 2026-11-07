/* Small deterministic teaching models. No shell execution, network or dependencies. */
(() => {
  'use strict';
  const button = (text, action) => {
    const b = document.createElement('button'); b.type = 'button';
    b.textContent = text; b.addEventListener('click', action); return b;
  };
  const text = (tag, value) => { const x = document.createElement(tag); x.textContent = value; return x; };
  const tools = host => { const x = document.createElement('div'); x.className = 'lab-controls'; host.append(x); return x; };
  const output = host => { const x = document.createElement('pre'); x.className = 'lab-output'; x.setAttribute('aria-live','polite'); host.append(x); return x; };
  const models = {
    toolchain(host) {
      host.append(text('h3','Which tool does which job?'));
      host.append(text('p','Choose a command and follow its role. These are labelled example responses, not results from your computer.'));
      const controls=tools(host), out=output(host);
      const commands={
        'rustup show':'Manages/selects installed Rust toolchains.\nExample: stable toolchain selected.\nCheck your real terminal for the installed version.',
        'rustc --version':'Asks the Rust compiler for its version.\nExample shape: rustc <version> (<commit> <date>).\nThis does not build or run your game.',
        'cargo check':'Cargo checks the package using the compiler.\nExample: Finished dev profile.\nNo game window is launched.',
        'cargo run':'Cargo builds changed code, then launches the executable.\nExample: Running target/debug/game\nYour main function now runs.'
      };
      Object.entries(commands).forEach(([command,message])=>controls.append(button(command,()=>{out.textContent='$ '+command+'\n\n'+message;})));
      out.textContent='Pick a command. Before clicking, predict whether it installs, checks, or runs anything.';
    },
    grid(host) {
      host.append(text('h3','A tiny occupancy grid'));
      host.append(text('p','Click an empty cell to place sand; click sand to erase it. Step moves each grain down by at most one cell in this model. The bottom stone row stays fixed.'));
      const controls=tools(host), board=document.createElement('div'); board.className='lab-grid'; board.style.setProperty('--columns','8');
      const out=output(host); host.insertBefore(board,out);
      const width=8,height=6; let cells;
      const reset=()=>{cells=Array(width*height).fill(0);for(let x=0;x<width;x++)cells[(height-1)*width+x]=2;cells[width+3]=1;render();};
      const render=()=>{
        board.replaceChildren();
        cells.forEach((value,index)=>{
          const x=index%width,y=Math.floor(index/width);
          const b=button('',()=>{if(y<height-1){cells[index]=value===1?0:1;render();}});
          b.className='grid-cell material-'+value;
          b.setAttribute('aria-label',`column ${x}, row ${y}, index ${index}: ${['air','sand','stone'][value]}`);
          b.title=`(${x}, ${y}) → ${index}`;
          b.append(text('span',String(index)));board.append(b);
        });
        out.textContent=`index = row × ${width} + column\nSand count: ${cells.filter(x=>x===1).length}\nClick cells to inspect coordinates and place material.`;
      };
      controls.append(button('Step one tick',()=>{
        for(let y=height-2;y>=0;y--)for(let x=0;x<width;x++){
          const i=y*width+x,j=i+width;
          if(cells[i]===1&&cells[j]===0){cells[j]=1;cells[i]=0;}
        }render();
      }),button('Reset grid',reset)); reset();
    },
    borrow(host) {
      host.append(text('h3','Who has access right now?'));
      host.append(text('p','This model separates access permission from ownership. The owner retains the cell while a borrow is active. Release a borrow before changing permission.'));
      let mode='none',row=30,reads=0;
      const controls=tools(host),out=output(host);
      const render=message=>{out.textContent=`Owner: main\nCell.row: ${row}\nBorrow: ${mode}\nShared reads: ${reads}\n\n${message}`;};
      controls.append(button('Borrow &Cell',()=>{if(mode==='mutable')return render('Exclusive access is in use: release it first.');mode='shared';reads++;render('A shared reader may inspect row, but may not mutate through &Cell.');}),
        button('Borrow &mut Cell',()=>{if(mode!=='none')return render('A borrow is still in use. End its use before exclusive mutable access.');mode='mutable';render('Exclusive mutable access is available. Try resetting the row.');}),
        button('Set row to 5',()=>{if(mode!=='mutable')return render('Mutation rejected in this model: no active &mut Cell permission.');row=5;render('The mutable borrow changed the original owned cell.');}),
        button('End borrow use',()=>{mode='none';reads=0;render('No borrow is in use. main still owns the same cell.');}),
        button('Reset example',()=>{mode='none';row=30;reads=0;render('Try reading, then mutating. Which permission is required?');}));
      render('Rust checks these relationships at compile time; these buttons visualize the rule.');
    },
    ticks(host) {
      host.append(text('h3','Pay for updates with elapsed time'));
      host.append(text('p','Add a frame duration. A tick costs 50 ms. This teaching clock runs at most two ticks per frame and discards excess whole-tick debt. It has no fast-forward calendar.'));
      let accumulator=0,steps=0,frames=0,discarded=0;
      const controls=tools(host),out=output(host);
      const render=()=>{out.textContent=`Frames drawn: ${frames}\nSimulation ticks: ${steps}\nFractional time retained: ${accumulator.toFixed(2)} ms\nDiscarded due ticks: ${discarded}\nCell row from ticks: ${steps*3}\n\nFrame count and simulation count are different quantities.`;};
      const frame=ms=>{frames++;accumulator+=ms;let work=0;while(accumulator>=50&&work<2){accumulator-=50;steps++;work++;}if(accumulator>=50){discarded+=Math.floor(accumulator/50);accumulator%=50;}render();};
      [16,30,50,250].forEach(ms=>controls.append(button(`+ ${ms} ms frame`,()=>frame(ms))));
      controls.append(button('Reset clock',()=>{accumulator=steps=frames=discarded=0;render();}));render();
    },
    snapshot(host) {
      host.append(text('h3','Crash before or after commitment'));
      host.append(text('p','A logical transaction model: generation 1 starts committed. New region files do not become current until the complete generation is validated and its commit reference is published.'));
      let stage=0; const controls=tools(host),out=output(host);
      const labels=['Old generation 1 committed','New region A written; B missing','New A + B and manifest validate','Commit reference published for generation 2'];
      const render=()=>{out.textContent=`Stage ${stage}: ${labels[stage]}\nCommitted generation: ${stage===3?2:1}\nNew generation complete: ${stage>=2?'yes':'no'}\n\nCrash recovery selects generation ${stage===3?2:1}.\nThis models protocol ordering, not actual filesystem durability.`;};
      controls.append(button('Advance one write stage',()=>{stage=Math.min(3,stage+1);render();}),button('Simulate crash / recover',()=>{const chosen=stage===3?2:1;render();out.textContent+=`\n\nRecovered a complete generation ${chosen}; never mixed A and B.`;}),button('Reset save',()=>{stage=0;render();}));render();
    }
  };
  document.querySelectorAll('[data-lab]').forEach(host=>{const model=models[host.dataset.lab];if(model)model(host);});
})();
