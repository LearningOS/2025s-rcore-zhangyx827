## lab2 实验报告

### 实现功能简述
| 系统调用名称 |   功能简述 |
| ---     | ----       |
|   ` sys_get_time` |  通过`current_user_token`得到当前用户的页表的token进而得到页表,接着遍历usize的每一个byte的地址，将虚拟页号转换为物理页号来应对可能出现的跨页访问，通过`get_bytes_array`得到物理页的内容并通过`page_offset`得到页内偏移来得到最物理地址，通过直接解引用指针来向每一个byte里面写入内容（通过bit mask） |
| `sys_trace` |   如果`trace_request`为0或者1，同`sys_get_time`一样，得到页表并读或者写页表来实现，不同的是还要访问的地址是否有效（满足位于高256GiB和低256GiB）判断页表项是否符合要求（是否有效，是否可读或者可写）如果`trace_request`为2，那么就通过在`task`模块加入的`CntSys`结构体会以及`get_count`函数来得到当前的用户的系统调用次数   | 
|  `mmap` | 在`task`模块中加入了`current_pagetable`来得到当前指向当前页表的物理地址的指针（相应地，`mm`的`TaskControlBlock`的实现中加入了`get_user_page_table`，在`mm`的`MemorySet`的实现中，加入了`get_page_table`以此来得到对页表的修改，其中物理内存的随便分配可以通过`alloc`实现  |

---
### 问答题

1. stride 算法原理非常简单，但是有一个比较大的问题。例如两个 pass = 10 的进程，使用 8bit 无符号整形储存 `stride`， `p1`.`stride` = 255, `p2`.`stride` = 250，在 `p2` 执行一个时间片后，理论上下一次应该 p1 执行 实际情况是轮到 `p1` 执行吗？为什么？
- 不是，`p2`运行一个时间片之后， 250 + 10 = 260 > 255 溢出导致 `p2`.`stride_new` = 4 < 255 实际上不会轮到`p1`。

2. 我们之前要求进程优先级 >= 2 其实就是为了解决这个问题。可以证明， 在***不考虑溢出***的情况下 , 在进程优先级全部 >= 2 的情况下，如果严格按照算法执行，那么 `STRIDE_MAX` – `STRIDE_MIN` <= `BigStride` / 2。
- 为什么？尝试简单说明（不要求严格证明）。
    - 因为

- 已知以上结论，***考虑溢出***的情况下，可以为 `Stride` 设计特别的比较器，让 BinaryHeap<Stride> 的 `pop` 方法能返回真正最小的 `Stride`。补全下列代码中的 `partial_cmp` 函数，假设两个 `Stride` 永远不会相等。
use core::cmp::Ordering;

```rust
struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0 < other.0 {
            return Some(Ordering::Less);
        } else if self.0 > other.0 {
            return Some(Ordering::Greater);
        } else {
            return None;
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}
```

---

### 荣誉准则

#### 1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

---

#### 2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

| 文件位置  |  参考内容   |	
| -------- | ------    | 
| os/src/syscall/fs.rs | 参考sys_write 通过 current_user_token 获取  用户页表物理地址并转换为页表的逻辑 |	
|os/src/mm/memory_set.rs |from_elf的实现中关于标志位的设置 |

---

调试方法参考
> GDB 调试技巧：用户态与内核态切换的调试方法参考自[代码调试](https://learningos.cn/rCore-Tutorial-Guide-2025S/honorcode.html)


#### 3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

#### 4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计

---


