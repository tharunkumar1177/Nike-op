using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.ViewModels;

public sealed class MacrosViewModel : ObservableObject
{
    private readonly Func<Task<bool>> _saveAsync;
    private ProfileWorkspace? _profile;
    private MacroDefinition? _selectedMacro;
    private string _macroSearch = string.Empty;
    private string _feedbackText = "Macro edits are saved through Runner and applied on profile activation.";

    public MacrosViewModel(Func<Task<bool>>? saveAsync = null)
    {
        _saveAsync = saveAsync ?? (() => Task.FromResult(true));
        NewMacroCommand = new RelayCommand(NewMacro, () => _profile is not null);
        AddActionCommand = new RelayCommand(AddAction, () => SelectedMacro is not null);
        DeleteStepCommand = new RelayCommand<MacroStep>(DeleteStep, step => SelectedMacro is not null && step is not null);
        DeleteMacroCommand = new RelayCommand(DeleteMacro, () => SelectedMacro is not null);
        DuplicateMacroCommand = new RelayCommand(DuplicateMacro, () => SelectedMacro is not null);
        SaveMacroCommand = new AsyncRelayCommand(SaveAsync, () => _profile is not null);
    }

    public ICommand NewMacroCommand { get; }
    public ICommand AddActionCommand { get; }
    public ICommand DeleteStepCommand { get; }
    public ICommand DeleteMacroCommand { get; }
    public ICommand DuplicateMacroCommand { get; }
    public ICommand SaveMacroCommand { get; }

    public ObservableCollection<MacroDefinition> Macros => _profile?.Macros ?? EmptyMacros;
    private static ObservableCollection<MacroDefinition> EmptyMacros { get; } = new();
    public IEnumerable<MacroDefinition> FilteredMacros => Macros.Where(FilterMacro);
    public bool HasMacros => Macros.Count > 0;
    public bool HasSelectedMacro => SelectedMacro is not null;
    public bool HasSteps => SelectedMacro?.Steps.Count > 0;
    public string? ValidationMessage => SelectedMacro is null ? null : MacroValidation.Validate(SelectedMacro, Macros);
    public bool HasValidationMessage => ValidationMessage is not null;

    public MacroDefinition? SelectedMacro
    {
        get => _selectedMacro;
        set
        {
            var previous = _selectedMacro;
            if (!SetProperty(ref _selectedMacro, value)) return;
            if (previous is not null)
            {
                previous.PropertyChanged -= SelectedMacroPropertyChanged;
                previous.Steps.CollectionChanged -= SelectedMacroStepsChanged;
                foreach (var step in previous.Steps) step.PropertyChanged -= StepPropertyChanged;
            }
            if (value is not null)
            {
                value.PropertyChanged += SelectedMacroPropertyChanged;
                value.Steps.CollectionChanged += SelectedMacroStepsChanged;
                foreach (var step in value.Steps) step.PropertyChanged += StepPropertyChanged;
            }
            OnPropertyChanged(nameof(HasSelectedMacro));
            NotifyStepsAndValidation();
            NotifyCommandState();
        }
    }

    public string MacroSearch
    {
        get => _macroSearch;
        set
        {
            var selected = SelectedMacro;
            if (!SetProperty(ref _macroSearch, value)) return;
            OnPropertyChanged(nameof(FilteredMacros));
            SelectedMacro = selected is not null && FilterMacro(selected) ? selected : FilteredMacros.FirstOrDefault();
        }
    }

    public string FeedbackText { get => _feedbackText; private set => SetProperty(ref _feedbackText, value); }

    public void LoadProfile(ProfileWorkspace profile)
    {
        _profile = profile;
        NotifyCollection();
        SelectedMacro = profile.Macros.FirstOrDefault();
        NotifyCommandState();
    }

    private bool FilterMacro(MacroDefinition macro) =>
        string.IsNullOrWhiteSpace(MacroSearch) || macro.Name.Contains(MacroSearch, StringComparison.OrdinalIgnoreCase);

    private void NewMacro()
    {
        if (_profile is null) return;
        var macro = new MacroDefinition(UniqueName("New macro"), MacroValidation.Unassigned, new[] { new MacroStep("Key press", string.Empty) });
        Macros.Add(macro);
        NotifyCollection();
        SelectedMacro = macro;
        FeedbackText = "Macro created. Set a key for its first action, then save.";
    }

    private void AddAction()
    {
        SelectedMacro?.Steps.Add(new MacroStep("Key press", string.Empty));
        FeedbackText = "Action added to the end of the sequence.";
    }

    private void DeleteStep(MacroStep? step)
    {
        if (SelectedMacro is null || step is null) return;
        SelectedMacro.Steps.Remove(step);
        FeedbackText = "Action removed. Save to apply the change.";
    }

    private void DeleteMacro()
    {
        if (SelectedMacro is null) return;
        var name = SelectedMacro.Name;
        var index = Macros.IndexOf(SelectedMacro);
        Macros.Remove(SelectedMacro);
        NotifyCollection();
        SelectedMacro = Macros.Count == 0 ? null : Macros[Math.Clamp(index, 0, Macros.Count - 1)];
        FeedbackText = $"Deleted {name}. Save to apply the change.";
    }

    private void DuplicateMacro()
    {
        if (SelectedMacro is null) return;
        var source = SelectedMacro;
        var duplicate = new MacroDefinition(
            UniqueName($"{source.Name} copy"),
            MacroValidation.Unassigned,
            source.Steps.Select(step => new MacroStep(step.Action, step.Value)))
        {
            IsEnabled = source.IsEnabled,
            RepeatMode = source.RepeatMode,
            RepeatCount = source.RepeatCount,
            StopKey = source.StopKey,
        };
        Macros.Add(duplicate);
        NotifyCollection();
        SelectedMacro = duplicate;
        FeedbackText = "Macro duplicated. Shortcuts are not copied, to avoid conflicts.";
    }

    private string UniqueName(string root)
    {
        var candidate = root;
        var suffix = 2;
        while (Macros.Any(macro => string.Equals(macro.Name.Trim(), candidate, StringComparison.OrdinalIgnoreCase)))
            candidate = $"{root} {suffix++}";
        return candidate;
    }

    private void SelectedMacroPropertyChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (args.PropertyName is nameof(MacroDefinition.Name) or nameof(MacroDefinition.Shortcut))
        {
            OnPropertyChanged(nameof(ValidationMessage));
            OnPropertyChanged(nameof(HasValidationMessage));
        }
    }

    private void SelectedMacroStepsChanged(object? sender, NotifyCollectionChangedEventArgs args)
    {
        if (args.OldItems is not null)
            foreach (MacroStep step in args.OldItems) step.PropertyChanged -= StepPropertyChanged;
        if (args.NewItems is not null)
            foreach (MacroStep step in args.NewItems) step.PropertyChanged += StepPropertyChanged;
        NotifyStepsAndValidation();
    }

    private void StepPropertyChanged(object? sender, PropertyChangedEventArgs args) => NotifyStepsAndValidation();

    private void NotifyStepsAndValidation()
    {
        OnPropertyChanged(nameof(HasSteps));
        OnPropertyChanged(nameof(ValidationMessage));
        OnPropertyChanged(nameof(HasValidationMessage));
    }

    private void NotifyCollection()
    {
        OnPropertyChanged(nameof(Macros));
        OnPropertyChanged(nameof(FilteredMacros));
        OnPropertyChanged(nameof(HasMacros));
    }

    private void NotifyCommandState()
    {
        ((RelayCommand)NewMacroCommand).NotifyCanExecuteChanged();
        ((RelayCommand)AddActionCommand).NotifyCanExecuteChanged();
        ((RelayCommand<MacroStep>)DeleteStepCommand).NotifyCanExecuteChanged();
        ((RelayCommand)DeleteMacroCommand).NotifyCanExecuteChanged();
        ((RelayCommand)DuplicateMacroCommand).NotifyCanExecuteChanged();
        ((AsyncRelayCommand)SaveMacroCommand).NotifyCanExecuteChanged();
    }

    private async Task SaveAsync()
    {
        foreach (var macro in Macros)
        {
            if (MacroValidation.Validate(macro, Macros) is not { } problem) continue;
            SelectedMacro = macro;
            FeedbackText = $"Not saved. {macro.Name}: {problem}";
            return;
        }
        FeedbackText = await _saveAsync() ? "Macro changes saved to Runner." : "Not saved. Runner is unavailable; your edits are kept in this window.";
    }
}
