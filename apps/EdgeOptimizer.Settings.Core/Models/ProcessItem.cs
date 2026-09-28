using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace EdgeOptimizer.Settings.Core.Models;

public sealed class ProcessItem : INotifyPropertyChanged
{
    private bool _isSelected;

    public ProcessItem(string name, string cpu, string memory, bool isSelected)
    {
        Name = name;
        Cpu = cpu;
        Memory = memory;
        _isSelected = isSelected;
    }

    public string Name { get; }
    public string Cpu { get; }
    public string Memory { get; }

    public bool IsSelected
    {
        get => _isSelected;
        set
        {
            if (_isSelected == value)
            {
                return;
            }

            _isSelected = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(IsSelected)));
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;
}
